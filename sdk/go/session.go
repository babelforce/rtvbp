package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go/proto"
)

var (
	ErrRequestTimeout          = fmt.Errorf("request: timeout")
	ErrRequestFailed           = fmt.Errorf("request: failed")
	ErrRequestValidationFailed = fmt.Errorf("request: client validation failed")
	ErrInvalidSessionState     = fmt.Errorf("session: invalid state")
)

type Session struct {
	id              string
	state           SessionState
	mu              sync.Mutex
	shCtx           *sessionHandlerCtx
	transport       Transport
	transportFunc   TransportFactory
	audio           *DuplexAudio
	closeOnce       sync.Once
	closeCh         chan struct{} // closeCh is a channel when closed will trigger shutdown of the session
	doneCh          chan struct{} // doneCh is a channel which will be closed whenever the connection fails on reading
	handler         SessionHandler
	pendingRequests map[string]*pendingRequest
	muPending       sync.Mutex
	logger          *slog.Logger
	requestTimeout  time.Duration
}

func (s *Session) ID() string {
	return s.id
}

// EventDispatch dispatches an event
func (s *Session) EventDispatch(_ context.Context, payload NamedEvent) error {
	evt := proto.NewEvent(payload.EventName(), payload)
	if err := evt.Validate(); err != nil {
		return fmt.Errorf("event validation failed: %w", err)
	}

	s.logger.Debug(
		"notify",
		"event_id", evt.ID,
		"event", evt.Event,
		"data", payload,
	)

	data, err := json.Marshal(evt)
	if err != nil {
		return err
	}

	if err := s.writeMsgData(data); err != nil {
		return fmt.Errorf("request [event=%s, id=%s]: %w", evt.Event, evt.ID, err)
	}

	return nil
}

func (s *Session) writeMsgData(data []byte) error {
	return s.transport.Write(data)
}

func (s *Session) doClose(ctx context.Context, cb func(ctx context.Context, h SHC) error) {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	// Check if already closing or closed
	state := s.State()
	if state == SessionStateClosed || state == SessionStateClosing {
		return
	}

	s.closeOnce.Do(func() {
		if cb != nil {
			if e2 := cb(ctx, s.shCtx); e2 != nil {
				s.logger.Error("session close callback failed", slog.Any("err", e2))
			}
		}
		close(s.closeCh)
	})
}

func (s *Session) Close(ctx context.Context, cb func(ctx context.Context, h SHC) error) error {

	s.doClose(ctx, cb)

	// wait until done
	select {
	case <-ctx.Done():
		return fmt.Errorf("session close failed due to timeout: %w", ctx.Err())
	case <-s.doneCh:
		return nil
	}
}

func (s *Session) endSession() {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	s.setState(SessionStateClosing)

	// close transport
	if s.transport != nil {
		if err := s.transport.Close(ctx); err != nil {
			s.logger.Error("failed to close transport", "err", err)
		}
	}

	s.setState(SessionStateClosed)
	close(s.doneCh)
	s.logger.Info("closed session")
}

func (s *Session) handleEvent(ctx context.Context, evt *proto.Event) {
	s.logger.Debug("handle event", slog.Any("evt", evt))

	if s.handler == nil {
		return
	}

	if err := s.handler.OnEvent(ctx, s.shCtx, evt); err != nil {
		s.logger.Error("handler.OnEvent failed", slog.Any("err", err))
	}
}

func (s *Session) handleRequest(ctx context.Context, req *proto.Request) {

	s.logger.Debug("handleRequest", slog.Any("req", req))
	if s.handler == nil {
		return
	}

	err := s.handler.OnRequest(ctx, s.shCtx, req)

	if err != nil {
		s.logger.Error("handler.OnRequest failed", slog.Any("request", req), slog.Any("err", err))

		if err2 := s.shCtx.Respond(ctx, req.NotOk(proto.ToResponseError(err))); err2 != nil {
			s.logger.Error("failed to respond to request", slog.Any("request", req), slog.Any("err", err))
		}
	}
}

func (s *Session) handleIncomingMessage(ctx context.Context, msg proto.Message) {

	switch m := msg.(type) {
	case *proto.Event:
		s.handleEvent(ctx, m)
	case *proto.Request:
		s.handleRequest(ctx, m)
	case *proto.Response:
		s.resolvePendingRequest(m)
	default:
		s.logger.Error("unknown message", slog.Any("msg", msg))
	}
}

func (s *Session) Run(
	ctx context.Context,
) <-chan error {
	var (
		done = make(chan error, 1)
	)

	// exit early without handler
	if s.handler == nil {
		done <- fmt.Errorf("no handler set")
		return done
	}

	// create transport
	if trans, err := s.transportFunc(ctx, s.audio.TransportRW()); err != nil {
		done <- err
		return done
	} else {
		s.transport = trans
	}

	go func() {
		defer func() {
			s.endSession()
			done <- nil
		}()
		for {
			select {
			case <-s.closeCh:
				return
			case <-ctx.Done():
				return
			}
		}

	}()

	// read incoming
	go func() {
		ctrlMsgInCh := s.transport.ReadChan()
		for {
			select {
			case <-s.doneCh:
				return
			case <-ctx.Done():
				return
			case p, ok := <-ctrlMsgInCh:
				if !ok {
					s.doClose(ctx, nil)
					return
				}

				msg, err := proto.ParseValidMessage(p.Data)
				if err != nil {
					s.logger.Error("parsing message json failed", slog.Any("err", err))
				} else {
					if p.ReceivedAt == 0 {
						msg.SetReceivedAt(time.Now().UnixMilli())
					} else {
						msg.SetReceivedAt(p.ReceivedAt)
					}
					go s.handleIncomingMessage(ctx, msg)
				}
			}
		}
	}()

	// Initialize
	go func() {
		if err := s.handler.OnBegin(ctx, s.shCtx); err != nil {
			beginErr := fmt.Errorf("handler.OnBegin() failed: %w", err)
			s.logger.Error("session init failed", slog.Any("err", beginErr))
			s.setState(SessionStateFailed)
			done <- beginErr
		} else {
			s.setState(SessionStateActive)
		}
	}()

	return done
}

// NewSession creates a new peer session
func NewSession(
	opts ...Option,
) *Session {
	// init options
	options := &sessionOptions{}
	withDefaults()(options)
	for _, opt := range opts {
		opt(options)
	}

	logger := options.logger.With(
		slog.String("session", options.id),
	)

	session := &Session{
		id:              options.id,
		state:           SessionStateInactive,
		transportFunc:   options.transport,
		closeCh:         make(chan struct{}),
		doneCh:          make(chan struct{}),
		pendingRequests: map[string]*pendingRequest{},
		handler:         options.handler,
		logger:          logger,
		audio:           NewSessionAudio(options.audioBufferSize),
		requestTimeout:  options.requestTimeout,
	}

	session.shCtx = &sessionHandlerCtx{
		sess: session,
		ha:   session.audio.toHandlerAudio(),
	}

	return session
}
