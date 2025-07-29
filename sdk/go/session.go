package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
	"log/slog"
	"sync"
	"time"
)

var (
	ErrRequestTimeout = fmt.Errorf("request: timeout")
)

type Session struct {
	id              string
	state           SessionState
	mu              sync.Mutex
	shCtx           *sessionHandlerCtx
	transport       Transport
	transportFunc   TransportFunc
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

// Notify sends a notification
func (s *Session) Notify(_ context.Context, payload NamedEvent) error {
	evt := proto.NewEvent("1", payload.EventName(), payload)

	s.logger.Debug(
		"Session.Notify()",
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
	return s.transport.Control().Write(data)
}

// CloseWithTimeout closes the client and the underlying transport
func (s *Session) CloseWithTimeout(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	return s.Close(ctx)
}

func (s *Session) Close(ctx context.Context) error {
	state := s.State()
	if state == SessionStateClosed || state == SessionStateClosing {
		return nil
	}

	s.closeOnce.Do(func() {
		s.logger.Info("closing session")
		s.setState(SessionStateClosing)
		close(s.closeCh)
	})

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

	// close transport
	if s.transport != nil {
		if err := s.transport.Close(ctx); err != nil {
			s.logger.Error("failed to close transport", "err", err)
		} else {
			s.logger.Info("transport closed")
		}
	}

	s.logger.Info("closed session")
	s.setState(SessionStateClosed)
	close(s.doneCh)
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

	if err := s.handler.OnRequest(ctx, s.shCtx, req); err != nil {
		s.logger.Error("handleRequest failed", slog.Any("request", req), slog.Any("err", err))
	}
}

func (s *Session) handleIncoming(ctx context.Context, msg sessionMessage) {
	s.logger.Debug("handleIncoming", slog.Any("msg", msg))

	if msg.Event != "" {
		s.handleEvent(ctx, &proto.Event{
			Version: msg.Version,
			ID:      msg.ID,
			Event:   msg.Event,
			Data:    msg.Data,
		})
	} else if msg.Method != "" {
		s.handleRequest(ctx, &proto.Request{
			Version: msg.Version,
			ID:      msg.ID,
			Method:  msg.Method,
			Params:  msg.Params,
		})
	} else if msg.Response != "" {
		s.resolvePendingRequest(&proto.Response{
			Version:  msg.Version,
			Response: msg.Response,
			Result:   msg.Result,
			Error:    msg.Error,
		})
	} else {
		s.logger.Error("unknown message type", slog.Any("msg", msg))
	}
}

func (s *Session) Run(
	ctx context.Context,
) <-chan error {
	var (
		done = make(chan error, 1)
	)

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
		ctrlMsgInCh := s.transport.Control().ReadChan()
		for {

			select {
			case <-s.doneCh:
				return
			case <-ctx.Done():
				return
			case data, ok := <-ctrlMsgInCh:
				if !ok {
					return
				}

				var msg sessionMessage
				err := json.Unmarshal(data, &msg)
				if err != nil {
					s.logger.Error("parsing message json failed", slog.Any("err", err))
				} else {
					go s.handleIncoming(ctx, msg)
				}
			}
		}
	}()

	if s.handler != nil {
		go func() {
			if err := s.handler.OnBegin(ctx, s.shCtx); err != nil {
				s.setState(SessionStateFailed)
				done <- fmt.Errorf("handler.OnBegin() failed: %w", err)
			} else {
				s.setState(SessionStateActive)
			}
		}()
	}

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
