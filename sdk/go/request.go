package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
)

type Validation interface {
	Validate() error
}

type NamedRequest interface {
	MethodName() string
}

type RequestHandler interface {
	MethodName() string
	Handle(ctx context.Context, hc SHC, req *proto.Request) error
}

type typedRequestHandler[REQ NamedRequest, RES any] struct {
	name string
	h    func(context.Context, SHC, REQ) (RES, error)
}

func (t *typedRequestHandler[REQ, RES]) MethodName() string {
	return t.name
}

// Handle handles a request
func (t *typedRequestHandler[REQ, RES]) Handle(ctx context.Context, h SHC, req *proto.Request) error {

	raw, err := json.Marshal(req.Params)
	if err != nil {
		return fmt.Errorf("marshal data: %w", err)
	}

	var params REQ
	if err := json.Unmarshal(raw, &params); err != nil {
		return fmt.Errorf("unmarshal into type: %w", err)
	}

	if v, ok := any(params).(Validation); ok {
		if err := v.Validate(); err != nil {
			return proto.NewBadRequestError(err)
		}
	}

	// handle request
	res, err := t.h(context.WithValue(ctx, "request", req), h, params)
	if err != nil {
		return err
	}

	if v, ok := any(res).(Validation); ok {
		if err := v.Validate(); err != nil {
			return proto.NewError(proto.ErrInternalServerError, fmt.Errorf("failed to produce valid response: %w", err))
		}
	}

	// respond to session
	if err := h.Respond(ctx, req.Ok(res)); err != nil {
		return fmt.Errorf("failed to respond: %w", err)
	}

	if prh, ok := any(params).(OnAfterReplyHook); ok {
		return prh.OnAfterReply(ctx, h)
	}

	return nil
}

func HandleRequest[REQ NamedRequest, RES any](handler func(context.Context, SHC, REQ) (RES, error)) RequestHandler {
	var zero REQ
	return &typedRequestHandler[REQ, RES]{
		name: zero.MethodName(),
		h:    handler,
	}
}
