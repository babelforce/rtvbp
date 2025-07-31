package rtvbp

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
)

type pendingRequest struct {
	id string
	ch chan *proto.Response
}

// newPendingRequest creates a new pending request
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

// resolvePendingRequest resolves a pending request
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

	ctx, cancel := context.WithTimeout(ctx, s.requestTimeout)
	defer cancel()

	req := proto.NewRequest(payload.MethodName(), payload)
	pr := s.newPendingRequest(req.ID)

	if err := s.sendMessage(req); err != nil {
		return nil, fmt.Errorf("%w: %w", ErrRequestValidationFailed, err)
	}

	// wait for response
	select {
	case <-ctx.Done():
		s.muPending.Lock()
		defer s.muPending.Unlock()
		delete(s.pendingRequests, req.ID)
		return nil, fmt.Errorf("request [method=%s, id=%s] failed: %w", req.Method, req.ID, ErrRequestTimeout)
	case resp := <-pr.ch:
		if !resp.Ok() {
			return nil, resp.Error
		}

		return resp, nil
	}
}
