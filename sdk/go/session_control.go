package rtvbp

import (
	"context"
	"errors"
	"io"
	"sync"
	"time"
)

func (s *Session) readControl(ctx context.Context) error {
	control := s.control()
	if control == nil {
		return io.ErrClosedPipe
	}
	for {
		received, err := control.Recv(ctx)
		if err != nil {
			return err
		}
		frame, err := s.envelope.Decode(received.Data)
		if err != nil {
			s.logger.Error("decode control frame", "error", err)
			continue
		}
		if received.ReceivedAt.IsZero() {
			frame.ReceivedAt = time.Now()
		} else {
			frame.ReceivedAt = received.ReceivedAt
		}
		if s.debug {
			debugFrame(s.id, frame, "in")
		}
		if frame.Kind == KindResponse {
			s.resolvePending(frame)
			continue
		}
		if !s.dispatch.push(frame) {
			return io.EOF
		}
	}
}

func (s *Session) dispatchControl(ctx context.Context) {
	for {
		frame, err := s.dispatch.pop(ctx)
		if err != nil {
			return
		}
		switch frame.Kind {
		case KindRequest:
			s.handleRequest(ctx, frame)
		case KindEvent:
			s.handleEvent(ctx, frame)
		}
	}
}

func (s *Session) handleRequest(ctx context.Context, frame ControlFrame) {
	reply := &replyState{requestID: frame.ID}
	handler := &sessionHandlerCtx{sess: s, ha: s.shCtx.ha, reply: reply}
	request := Request{
		ID:         frame.ID,
		Method:     frame.Method,
		Payload:    cloneRaw(frame.Payload),
		ReceivedAt: frame.ReceivedAt,
	}
	err := s.handler.OnRequest(ctx, handler, request)
	if err != nil {
		if !responseSent(handler) {
			if responseErr := handler.Respond(ctx, Response{Err: wireErrorFrom(err)}); responseErr != nil && !errors.Is(responseErr, ErrSessionClosed) {
				s.logger.Error("send request error response", "method", frame.Method, "error", responseErr)
			}
		}
		return
	}
	if reply.status.Load() == replyUnclaimed {
		err := errors.New("request handler returned without responding or deferring")
		if responseErr := handler.Respond(ctx, Response{Err: wireErrorFrom(err)}); responseErr != nil && !errors.Is(responseErr, ErrSessionClosed) {
			s.logger.Error("send missing response error", "method", frame.Method, "error", responseErr)
		}
	}
}

func (s *Session) handleEvent(ctx context.Context, frame ControlFrame) {
	event := Event{
		ID:         frame.ID,
		Name:       frame.Method,
		Payload:    cloneRaw(frame.Payload),
		ReceivedAt: frame.ReceivedAt,
	}
	if err := s.handler.OnEvent(ctx, s.shCtx, event); err != nil {
		s.logger.Error("handle event", "event", frame.Method, "error", err)
	}
}

func (s *Session) control() ControlChannel {
	s.transportMu.RLock()
	defer s.transportMu.RUnlock()
	if s.transport == nil {
		return nil
	}
	return s.transport.Control()
}

func (s *Session) sendFrame(ctx context.Context, frame ControlFrame) error {
	if s.closing.Load() {
		return ErrSessionClosed
	}
	control := s.control()
	if control == nil {
		return ErrSessionClosed
	}
	data, err := s.envelope.Encode(frame)
	if err != nil {
		return err
	}
	if s.debug {
		debugFrame(s.id, frame, "out")
	}
	if err := control.Send(ctx, data); err != nil {
		return err
	}
	return nil
}

func (s *Session) sendResponse(ctx context.Context, correlationID string, response Response) error {
	return s.sendFrame(ctx, ControlFrame{
		Kind:     KindResponse,
		CorrelID: correlationID,
		Payload:  cloneRaw(response.Payload),
		Err:      cloneWireError(response.Err),
	})
}

func cloneRaw(raw []byte) []byte {
	if raw == nil {
		return nil
	}
	return append([]byte(nil), raw...)
}

func cloneWireError(wire *WireError) *WireError {
	if wire == nil {
		return nil
	}
	return &WireError{Code: wire.Code, Message: wire.Message, Data: cloneRaw(wire.Data)}
}

type dispatchQueue struct {
	mu     sync.Mutex
	frames []ControlFrame
	closed bool
	ready  chan struct{}
}

func newDispatchQueue() *dispatchQueue {
	return &dispatchQueue{ready: make(chan struct{}, 1)}
}

func (q *dispatchQueue) push(frame ControlFrame) bool {
	q.mu.Lock()
	defer q.mu.Unlock()
	if q.closed {
		return false
	}
	q.frames = append(q.frames, frame)
	signalQueue(q.ready)
	return true
}

func (q *dispatchQueue) pop(ctx context.Context) (ControlFrame, error) {
	for {
		q.mu.Lock()
		if len(q.frames) != 0 {
			frame := q.frames[0]
			q.frames[0] = ControlFrame{}
			q.frames = q.frames[1:]
			q.mu.Unlock()
			return frame, nil
		}
		if q.closed {
			q.mu.Unlock()
			return ControlFrame{}, io.EOF
		}
		ready := q.ready
		q.mu.Unlock()
		select {
		case <-ctx.Done():
			return ControlFrame{}, ctx.Err()
		case <-ready:
		}
	}
}

func (q *dispatchQueue) close() {
	q.mu.Lock()
	q.closed = true
	q.mu.Unlock()
	signalQueue(q.ready)
}

func signalQueue(ready chan struct{}) {
	select {
	case ready <- struct{}{}:
	default:
	}
}
