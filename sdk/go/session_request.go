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

// cancelPending atomically removes a request only when no response or shutdown
// failure has already claimed it. A false return means the completion path won
// and has published exactly one result to expected.result.
func (s *Session) cancelPending(id string, expected *pendingRequest) bool {
	s.pendingMu.Lock()
	defer s.pendingMu.Unlock()
	if s.pending[id] == expected {
		delete(s.pending, id)
		return true
	}
	return false
}

func (s *Session) completePending(id string, result pendingResult) bool {
	s.pendingMu.Lock()
	defer s.pendingMu.Unlock()
	pending := s.pending[id]
	if pending == nil {
		return false
	}
	delete(s.pending, id)
	// The cell is buffered and only the map owner may publish, so this cannot
	// block. Publishing while holding pendingMu makes completion indivisible
	// from cancellation's point of view.
	pending.result <- result
	return true
}

func (s *Session) resolvePending(frame ControlFrame) {
	response := Response{Payload: cloneRaw(frame.Payload), Err: cloneWireError(frame.Err)}
	result := pendingResult{response: response}
	if response.Err != nil {
		result.err = &RemoteError{WireError: *response.Err}
	}
	s.completePending(frame.CorrelID, result)
}

func (s *Session) failPending(cause error) {
	s.pendingMu.Lock()
	defer s.pendingMu.Unlock()
	for id, request := range s.pending {
		delete(s.pending, id)
		request.result <- pendingResult{err: cause}
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
		if !s.cancelPending(id, pending) {
			result := <-pending.result
			return result.response, result.err
		}
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
		if !s.cancelPending(id, pending) {
			result := <-pending.result
			return result.response, result.err
		}
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
