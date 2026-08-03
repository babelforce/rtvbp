package rtvbp

import (
	"context"
	"errors"
	"fmt"
)

type pendingResult struct {
	response Response
	err      error
}

type pendingRequest struct {
	id     string
	result chan pendingResult
}

func (s *Session) registerPending(id string) (*pendingRequest, error) {
	if id == "" {
		return nil, errors.New("rtvbp: id generator returned an empty id")
	}
	pending := &pendingRequest{id: id, result: make(chan pendingResult, 1)}
	s.pendingMu.Lock()
	if _, exists := s.pending[id]; exists {
		s.pendingMu.Unlock()
		return nil, fmt.Errorf("rtvbp: id generator returned duplicate id %q", id)
	}
	s.pending[id] = pending
	s.pendingMu.Unlock()
	return pending, nil
}

func (s *Session) removePending(id string, expected *pendingRequest) {
	s.pendingMu.Lock()
	if s.pending[id] == expected {
		delete(s.pending, id)
	}
	s.pendingMu.Unlock()
}

func (s *Session) resolvePending(frame ControlFrame) {
	s.pendingMu.Lock()
	pending := s.pending[frame.CorrelID]
	if pending != nil {
		delete(s.pending, frame.CorrelID)
	}
	s.pendingMu.Unlock()
	if pending == nil {
		return
	}
	response := Response{Payload: cloneRaw(frame.Payload), Err: cloneWireError(frame.Err)}
	result := pendingResult{response: response}
	if response.Err != nil {
		result.err = &RemoteError{WireError: *response.Err}
	}
	select {
	case pending.result <- result:
	default:
	}
}

func (s *Session) failPending(cause error) {
	s.pendingMu.Lock()
	pending := s.pending
	s.pending = make(map[string]*pendingRequest)
	s.pendingMu.Unlock()
	for _, request := range pending {
		select {
		case request.result <- pendingResult{err: cause}:
		default:
		}
	}
}

func (s *Session) Request(ctx context.Context, payload NamedRequest) (Response, error) {
	if payload == nil || isNil(payload) {
		return Response{}, fmt.Errorf("%w: request is nil", ErrRequestValidationFailed)
	}
	if validation, ok := payload.(Validation); ok {
		if err := validation.Validate(); err != nil {
			return Response{}, fmt.Errorf("%w: %w", ErrRequestValidationFailed, err)
		}
	}
	encoded, err := marshalPayload(payload)
	if err != nil {
		return Response{}, fmt.Errorf("%w: %w", ErrRequestValidationFailed, err)
	}
	if s.closing.Load() {
		return Response{}, ErrSessionClosed
	}
	id := s.idGenerator()
	pending, err := s.registerPending(id)
	if err != nil {
		return Response{}, fmt.Errorf("%w: %w", ErrRequestFailed, err)
	}
	frame := ControlFrame{Kind: KindRequest, ID: id, Method: payload.MethodName(), Payload: encoded}
	if err := s.sendFrame(ctx, frame); err != nil {
		s.removePending(id, pending)
		if errors.Is(err, ErrSessionClosed) {
			return Response{}, ErrSessionClosed
		}
		return Response{}, fmt.Errorf("%w: %w", ErrRequestFailed, err)
	}

	waitCtx := ctx
	cancel := func() {}
	internalTimeout := false
	if s.requestLimit > 0 {
		var cancelContext context.CancelFunc
		waitCtx, cancelContext = context.WithTimeout(ctx, s.requestLimit)
		cancel = cancelContext
		internalTimeout = true
	}
	defer cancel()

	select {
	case result := <-pending.result:
		return result.response, result.err
	case <-waitCtx.Done():
		select {
		case result := <-pending.result:
			return result.response, result.err
		default:
		}
		s.removePending(id, pending)
		if internalTimeout && errors.Is(waitCtx.Err(), context.DeadlineExceeded) && ctx.Err() == nil {
			return Response{}, fmt.Errorf("request method=%s id=%s: %w", payload.MethodName(), id, ErrRequestTimeout)
		}
		return Response{}, waitCtx.Err()
	}
}

func (s *Session) EventDispatch(ctx context.Context, payload NamedEvent) error {
	if payload == nil || isNil(payload) {
		return fmt.Errorf("%w: event is nil", ErrRequestValidationFailed)
	}
	if validation, ok := payload.(Validation); ok {
		if err := validation.Validate(); err != nil {
			return fmt.Errorf("%w: %w", ErrRequestValidationFailed, err)
		}
	}
	encoded, err := marshalPayload(payload)
	if err != nil {
		return fmt.Errorf("encode event: %w", err)
	}
	return s.sendFrame(ctx, ControlFrame{
		Kind:    KindEvent,
		ID:      s.idGenerator(),
		Method:  payload.EventName(),
		Payload: encoded,
	})
}
