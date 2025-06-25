package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
	"io"
	"log/slog"
	"sync"
	"time"
)

var (
	ErrRequestTimeout = fmt.Errorf("request: timeout")
)

type sessionMessage struct {
	Version  string               `json:"version,omitempty"`
	ID       string               `json:"id,omitempty"`
	Method   string               `json:"method,omitempty"`
	Response string               `json:"response,omitempty"`
	Event    string               `json:"event,omitempty"`
	Data     any                  `json:"data,omitempty"`
	Params   any                  `json:"params,omitempty"`
	Result   any                  `json:"result,omitempty"`
	Error    *proto.ResponseError `json:"error,omitempty"`
}

type pendingRequest struct {
	id string
	ch chan *proto.Response
}

type Session struct {
	id              string
	shCtx           *sessionHandlerCtx
	transport       Transport
	transportFunc   func(ctx context.Context) (Transport, error)
	closeOnce       sync.Once
	close           chan struct{} // close is a channel when closed will trigger shutdown of the session
	done            chan struct{} // done is a channel which will be closed whenever the connection fails on reading
	handler         SessionHandler
	pendingRequests map[string]*pendingRequest
	muPending       sync.Mutex
	out             chan []byte
	logger          *slog.Logger
}

func (s *Session) Audio() io.ReadWriter {
	return s.transport
}

// Notify sends a notification
func (s *Session) Notify(ctx context.Context, payload NamedEvent) error {
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

	if err := s.writeMsgData(ctx, data); err != nil {
		return fmt.Errorf("request [event=%s, id=%s]: %w", evt.Event, evt.ID, err)
	}
	return nil
}

func (s *Session) writeMsgData(ctx context.Context, data []byte) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	case s.out <- data:
		return nil
	}
}

func (s *Session) newPendingRequest(id string) *pendingRequest {
	s.muPending.Lock()
	defer s.muPending.Unlock()

	pr := &pendingRequest{
		id: id,
		ch: make(chan *proto.Response, 1),
	}

	s.pendingRequests[id] = pr

	return pr
}

func (s *Session) resolvePendingRequest(resp *proto.Response) {
	s.muPending.Lock()
	defer s.muPending.Unlock()

	pr, ok := s.pendingRequests[resp.Response]
	if !ok {
		return
	}

	pr.ch <- resp

	delete(s.pendingRequests, resp.Response)
}

// Request sends a request
func (s *Session) Request(ctx context.Context, payload NamedRequest) (*proto.Response, error) {
	req := proto.NewRequest("1", payload.MethodName(), payload)

	slog.Debug(
		"Session.Request()",
		"request_id", req.ID,
		"method", req.Method,
		"params", req.Params,
	)

	data, err := json.Marshal(req)
	if err != nil {
		return nil, err
	}

	pendingRequest := s.newPendingRequest(req.ID)

	if err := s.writeMsgData(ctx, data); err != nil {
		return nil, fmt.Errorf("request [method=%s, id=%s]: %w", req.Method, req.ID, err)
	}

	// wait for response
	select {
	case <-ctx.Done():
		return nil, fmt.Errorf("request [method=%s, id=%s] failed: %w", req.Method, req.ID, ErrRequestTimeout)
	case resp := <-pendingRequest.ch:
		if !resp.Ok() {
			return nil, resp.Error
		}

		return resp, nil
	}
}

// CloseTimeout closes the client and the underlying transport
func (s *Session) CloseTimeout(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	return s.CloseContext(ctx)
}

func (s *Session) CloseContext(ctx context.Context) error {

	s.closeOnce.Do(func() {
		s.logger.Info("closing session")
		close(s.close)
	})

	// wait until done
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-s.done:
		return nil
	}
}

func (s *Session) endSession() {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if s.handler != nil {
		if err := s.handler.OnEnd(ctx, s.shCtx); err != nil {
			s.logger.Error("handler.OnEnd() failed", slog.Any("err", err))
		}
	}

	// close transport
	if s.transport != nil {
		if err := s.transport.Close(ctx); err != nil {
			s.logger.Error("failed to close transport", "err", err)
		}
	}

	close(s.done)

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

	if err := s.handler.OnRequest(ctx, s.shCtx, req); err != nil {
		s.logger.Error("handleRequest failed", slog.Any("err", err))
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
) (err error) {
	defer s.endSession()

	// init transport

	if t, err := s.transportFunc(ctx); err != nil {
		return err
	} else {
		s.transport = t
	}

	var (
		logger = s.logger
	)

	if s.handler != nil {
		go func() {
			if err := s.handler.OnBegin(ctx, s.shCtx); err != nil {
				logger.Error("OnBegin() failed", slog.Any("err", err))
				return
			}
		}()
	}

	transportMsgInChan := s.transport.Control().ReadChan()
	transportClosedChan := s.transport.Closed()
	for {

		select {
		case <-s.close:
			return nil
		case <-ctx.Done():
			return nil
		case <-transportClosedChan:
			return nil
		case data, ok := <-s.out:
			if !ok {
				return
			}
			s.transport.Control().WriteChan() <- data
		case data, ok := <-transportMsgInChan:
			if !ok {
				s.logger.Debug("Session.Run() control channel closed")
				return nil
			}

			var msg sessionMessage
			if err := json.Unmarshal(data, &msg); err != nil {
				s.logger.Error("parsing message json failed", slog.Any("err", err))
			} else {
				go s.handleIncoming(ctx, msg)
			}
		}
	}
}

// NewSession creates a new peer for a transport and config
func NewSession(
	transportFunc func(ctx context.Context) (Transport, error),
	handler SessionHandler,
) *Session {
	sessionID := proto.ID()

	logger := slog.Default().With(
		slog.String("component", "session"),
		slog.String("id", sessionID),
	)

	session := &Session{
		id:              sessionID,
		transportFunc:   transportFunc,
		close:           make(chan struct{}),
		done:            make(chan struct{}),
		pendingRequests: map[string]*pendingRequest{},
		handler:         handler,
		logger:          logger,
		out:             make(chan []byte, 32),
	}

	session.shCtx = &sessionHandlerCtx{sess: session}

	return session
}
