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
	Handle(ctx context.Context, hc SHC, req *proto.Request) error
}

type typedRequestHandler[REQ NamedRequest, RES any] struct {
	name string
	h    func(context.Context, SHC, REQ) (RES, error)
}

func (t *typedRequestHandler[REQ, RES]) MethodName() string {
	return t.name
}

func (t *typedRequestHandler[REQ, RES]) Handle(ctx context.Context, h SHC, req *proto.Request) error {

	raw, err := json.Marshal(req.Params)
	if err != nil {
		return fmt.Errorf("marshal data: %w", err)
	}

	var params REQ
	if err := json.Unmarshal(raw, &params); err != nil {
		return fmt.Errorf("unmarshal into type: %w", err)
	}

	res, err := t.h(ctx, h, params)
	if err != nil {
		return err
	}

	if err := h.Respond(ctx, req.Ok(res)); err != nil {
		return err
	}

	var a any = params
	if prh, ok := a.(PostResponseHook); ok {
		println("IS POST")
		return prh.PostResponseHook(ctx, h)
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
