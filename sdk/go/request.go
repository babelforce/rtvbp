package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
)

type NamedRequest interface {
	MethodName() string
}

type RequestHandler interface {
	MethodName() string
	Handle(ctx context.Context, hc HandlerCtx, req *proto.Request) (*proto.Response, error)
}

type typedRequestHandler[I NamedRequest, O any] struct {
	name string
	h    func(context.Context, HandlerCtx, I) (*O, error)
}

func (t *typedRequestHandler[I, O]) MethodName() string {
	return t.name
}

func (t *typedRequestHandler[I, O]) Handle(ctx context.Context, h HandlerCtx, req *proto.Request) (*proto.Response, error) {
	raw, err := json.Marshal(req.Params)
	if err != nil {
		return nil, fmt.Errorf("marshal data: %w", err)
	}

	var params I
	if err := json.Unmarshal(raw, &params); err != nil {
		return nil, fmt.Errorf("unmarshal into type: %w", err)
	}

	out, err := t.h(ctx, h, params)
	if err != nil {
		return nil, fmt.Errorf("handle request: %w", err)
	}

	// TODO: if out is IntoResponseError

	// TODO: howto handle errors ?

	return &proto.Response{
		Version:  req.Version,
		Response: req.ID,
		Result:   out,
	}, nil
}

func HandleRequest[T NamedRequest, O any](handler func(context.Context, HandlerCtx, T) (*O, error)) RequestHandler {
	var zero T
	return &typedRequestHandler[T, O]{
		name: zero.MethodName(),
		h:    handler,
	}
}
