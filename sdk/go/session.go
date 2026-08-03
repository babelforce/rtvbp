package rtvbp

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"
)

var ErrSessionAlreadyRun = errors.New("rtvbp: session has already run")

type CloseHandler func(ctx context.Context) error

type stopRequest struct {
	cause  error
	failed bool
}

type Session struct {
	id          string
	envelope    Envelope
	idGenerator IDGenerator

	stateMu sync.RWMutex
	state   SessionState

	runMu      sync.Mutex
	runStarted bool
	done       chan error
	finalized  chan struct{}
	stop       chan stopRequest
	closing    atomic.Bool

	transportMu sync.RWMutex
	transport   Transport
	factory     TransportFactory

	handler      SessionHandler
	shCtx        *sessionHandlerCtx
	audio        *audioStream
	mediaMu      sync.Mutex
	mediaState   audioBindState
	media        MediaChannel
	mediaWG      sync.WaitGroup
	dispatch     *dispatchQueue
	requestLimit time.Duration
	closeTimeout time.Duration
	keepalive    KeepalivePolicy

	pendingMu sync.Mutex
	pending   map[string]*pendingRequest

	closeHandlersMu sync.Mutex
	closeHandlers   []CloseHandler
	logger          *slog.Logger
	debug           bool
}

func NewSession(envelope Envelope, options ...Option) *Session {
	config := &sessionOptions{}
	withDefaults()(config)
	for _, option := range options {
		option(config)
	}
	if config.logger == nil {
		config.logger = slog.Default()
	}

	session := &Session{
		id:            config.id,
		envelope:      envelope,
		idGenerator:   config.idGenerator,
		state:         SessionStateInactive,
		done:          make(chan error, 1),
		finalized:     make(chan struct{}),
		stop:          make(chan stopRequest, 1),
		factory:       config.transport,
		handler:       config.handler,
		dispatch:      newDispatchQueue(),
		requestLimit:  config.requestTimeout,
		closeTimeout:  config.closeTimeout,
		keepalive:     config.keepalive,
		pending:       make(map[string]*pendingRequest),
		closeHandlers: make([]CloseHandler, 0),
		logger:        config.logger.With(slog.String("session", config.id)),
		debug:         config.debug,
		audio:         newAudioStream(config.audioBufferSize),
	}
	var handlerAudio HandlerAudio = session.audio
	if config.streamObserver != nil {
		handlerAudio = &ObservableAudio{s: session, ha: session.audio, o: *config.streamObserver}
	}
	session.shCtx = &sessionHandlerCtx{sess: session, ha: handlerAudio}
	return session
}

func (s *Session) ID() string { return s.id }

func (s *Session) Run(parent context.Context) <-chan error {
	s.runMu.Lock()
	if s.runStarted {
		s.runMu.Unlock()
		result := make(chan error, 1)
		result <- ErrSessionAlreadyRun
		close(result)
		return result
	}
	s.runStarted = true
	s.setState(SessionStateConnecting)
	s.runMu.Unlock()

	go s.supervise(parent)
	return s.done
}

func (s *Session) supervise(parent context.Context) {
	if parent == nil {
		parent = context.Background()
	}
	var terminal stopRequest
	if s.envelope == nil {
		terminal = stopRequest{cause: errors.New("rtvbp: envelope is required"), failed: true}
		s.complete(s.shutdown(terminal, nil))
		return
	}
	if s.factory == nil {
		terminal = stopRequest{cause: errors.New("rtvbp: transport factory is required"), failed: true}
		s.complete(s.shutdown(terminal, nil))
		return
	}
	if s.handler == nil {
		terminal = stopRequest{cause: errors.New("rtvbp: session handler is required"), failed: true}
		s.complete(s.shutdown(terminal, nil))
		return
	}
	if s.idGenerator == nil {
		terminal = stopRequest{cause: errors.New("rtvbp: id generator is required"), failed: true}
		s.complete(s.shutdown(terminal, nil))
		return
	}
	if err := s.keepalive.Validate(); err != nil {
		terminal = stopRequest{cause: err, failed: true}
		s.complete(s.shutdown(terminal, nil))
		return
	}

	transport, terminal := s.createTransport(parent)
	if transport == nil {
		s.complete(s.shutdown(terminal, nil))
		return
	}
	s.transportMu.Lock()
	s.transport = transport
	s.transportMu.Unlock()

	taskCtx, cancelTasks := context.WithCancel(context.WithoutCancel(parent))
	workerResult := make(chan stopRequest, 4)
	var workers sync.WaitGroup
	workers.Add(2)
	go func() {
		defer workers.Done()
		err := s.readControl(taskCtx)
		if taskCtx.Err() == nil {
			if err == nil {
				err = errors.New("rtvbp: control reader stopped without an error")
			}
			workerResult <- stopFromTransport(err)
		}
	}()
	go func() {
		defer workers.Done()
		s.dispatchControl(taskCtx)
	}()
	if monitor, ok := transport.(KeepaliveTransport); ok && s.keepalive.Enabled() {
		workers.Add(1)
		go func() {
			defer workers.Done()
			err := monitor.MonitorKeepalive(taskCtx, s.keepalive)
			if taskCtx.Err() == nil {
				if err == nil {
					err = errors.New("rtvbp: keepalive monitor stopped without an error")
				}
				workerResult <- stopRequest{cause: err, failed: true}
			}
		}()
	}

	beginResult := make(chan error, 1)
	workers.Add(1)
	go func() {
		defer workers.Done()
		beginResult <- s.handler.OnBegin(taskCtx, s.shCtx)
	}()

	select {
	case err := <-beginResult:
		if err != nil {
			terminal = stopRequest{cause: fmt.Errorf("handler OnBegin: %w", err), failed: true}
		} else {
			s.setState(SessionStateActive)
		}
	case terminal = <-workerResult:
	case terminal = <-s.stop:
	case <-parent.Done():
		terminal = stopRequest{}
	}

	if terminal.cause == nil && !terminal.failed && s.State() == SessionStateActive {
		select {
		case terminal = <-workerResult:
		case terminal = <-s.stop:
		case <-parent.Done():
			terminal = stopRequest{}
		}
	}

	terminal = s.beginShutdown(terminal)
	cancelTasks()
	s.dispatch.close()
	terminal = s.closeResources(terminal, transport)

	workersDone := make(chan struct{})
	go func() {
		workers.Wait()
		s.mediaWG.Wait()
		close(workersDone)
	}()
	select {
	case <-workersDone:
	case <-time.After(s.teardownTimeout()):
		if !terminal.failed {
			terminal = stopRequest{cause: errors.New("rtvbp: session workers did not stop"), failed: true}
		}
	}
	s.complete(terminal)
}

func (s *Session) shutdown(terminal stopRequest, transport Transport) stopRequest {
	terminal = s.beginShutdown(terminal)
	s.dispatch.close()
	return s.closeResources(terminal, transport)
}

func (s *Session) beginShutdown(terminal stopRequest) stopRequest {
	s.closing.Store(true)
	if s.State() != SessionStateClosing {
		s.setState(SessionStateClosing)
	}

	pendingError := terminal.cause
	if pendingError == nil {
		pendingError = ErrSessionClosed
	}
	s.failPending(pendingError)
	_ = s.audio.Close()
	return terminal
}

func (s *Session) closeResources(terminal stopRequest, transport Transport) stopRequest {
	if err := s.closeAudioMedia(); err != nil {
		terminal.cause = errors.Join(terminal.cause, fmt.Errorf("close audio media: %w", err))
		terminal.failed = true
	}

	if transport != nil {
		if err := s.closeTransport(transport); err != nil {
			terminal.cause = errors.Join(terminal.cause, fmt.Errorf("close transport: %w", err))
			terminal.failed = true
		}
	}

	s.closeHandlersMu.Lock()
	handlers := append([]CloseHandler(nil), s.closeHandlers...)
	s.closeHandlersMu.Unlock()
	for _, handler := range handlers {
		ctx, cancel := context.WithTimeout(context.Background(), s.teardownTimeout())
		if err := handler(ctx); err != nil {
			s.logger.Error("session close handler failed", slog.Any("err", err))
		}
		cancel()
	}

	return terminal
}

type transportResult struct {
	transport Transport
	err       error
}

func (s *Session) createTransport(parent context.Context) (Transport, stopRequest) {
	ctx, cancel := context.WithCancel(parent)
	result := make(chan transportResult)
	abandoned := make(chan struct{})
	go func() {
		transport, err := s.factory(ctx, s.envelope)
		select {
		case result <- transportResult{transport: transport, err: err}:
		case <-abandoned:
			if transport != nil {
				if closeErr := s.closeTransport(transport); closeErr != nil {
					s.logger.Error("close late transport factory result", slog.Any("err", closeErr))
				}
			}
		}
	}()

	select {
	case created := <-result:
		cancel()
		return s.acceptTransportResult(created, parent)
	case terminal := <-s.stop:
		cancel()
		return nil, s.joinCanceledTransportFactory(ctx, result, abandoned, terminal)
	case <-parent.Done():
		cancel()
		return nil, s.joinCanceledTransportFactory(ctx, result, abandoned, stopRequest{})
	}
}

func (s *Session) acceptTransportResult(created transportResult, parent context.Context) (Transport, stopRequest) {
	if created.err == nil && created.transport != nil {
		return created.transport, stopRequest{}
	}
	terminal := stopRequest{}
	if created.err != nil {
		if parent.Err() == nil || !errors.Is(created.err, parent.Err()) {
			terminal.cause = fmt.Errorf("create transport: %w", created.err)
			terminal.failed = true
		}
	} else {
		terminal.cause = errors.New("rtvbp: transport factory returned nil")
		terminal.failed = true
	}
	if created.transport != nil {
		if err := s.closeTransport(created.transport); err != nil {
			terminal.cause = errors.Join(terminal.cause, fmt.Errorf("close partial transport: %w", err))
			terminal.failed = true
		}
	}
	return nil, terminal
}

func (s *Session) joinCanceledTransportFactory(ctx context.Context, result <-chan transportResult, abandoned chan struct{}, terminal stopRequest) stopRequest {
	timer := time.NewTimer(s.teardownTimeout())
	defer timer.Stop()
	select {
	case created := <-result:
		if created.transport != nil {
			if err := s.closeTransport(created.transport); err != nil {
				terminal.cause = errors.Join(terminal.cause, fmt.Errorf("close canceled transport: %w", err))
				terminal.failed = true
			}
		}
		if created.err != nil && (ctx.Err() == nil || !errors.Is(created.err, ctx.Err())) {
			terminal.cause = errors.Join(terminal.cause, fmt.Errorf("create transport during cancellation: %w", created.err))
			terminal.failed = true
		}
	case <-timer.C:
		close(abandoned)
		terminal.cause = errors.Join(terminal.cause, errors.New("rtvbp: transport factory did not stop"))
		terminal.failed = true
	}
	return terminal
}

func (s *Session) closeTransport(transport Transport) error {
	ctx, cancel := context.WithTimeout(context.Background(), s.teardownTimeout())
	defer cancel()
	return transport.Close(ctx)
}

func (s *Session) teardownTimeout() time.Duration {
	if s.closeTimeout > 0 {
		return s.closeTimeout
	}
	return 5 * time.Second
}

func (s *Session) complete(terminal stopRequest) {
	if terminal.failed {
		s.setState(SessionStateFailed)
	} else {
		s.setState(SessionStateClosed)
	}
	s.done <- terminal.cause
	close(s.done)
	close(s.finalized)
}

func (s *Session) Close(ctx context.Context) error {
	s.runMu.Lock()
	started := s.runStarted
	s.runMu.Unlock()
	if !started {
		return ErrSessionClosed
	}
	s.requestStop(nil, false)
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-s.finalized:
		return nil
	}
}

func (s *Session) requestStop(cause error, failed bool) {
	s.closing.Store(true)
	select {
	case s.stop <- stopRequest{cause: cause, failed: failed}:
	default:
	}
}

func (s *Session) OnClose(handler CloseHandler) {
	s.closeHandlersMu.Lock()
	s.closeHandlers = append(s.closeHandlers, handler)
	s.closeHandlersMu.Unlock()
}

func stopFromTransport(err error) stopRequest {
	if errors.Is(err, io.EOF) {
		return stopRequest{}
	}
	return stopRequest{cause: fmt.Errorf("receive control: %w", err), failed: true}
}
