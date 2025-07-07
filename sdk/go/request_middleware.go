package rtvbp

import (
	"context"
	"github.com/babelforce/rtvbp-go/proto"
)

type RequestMiddlewareFunc func(ctx context.Context, h SHC, req *proto.Request) error

type requestMiddleware struct {
	next RequestHandler
	fn   RequestMiddlewareFunc
}

func (m *requestMiddleware) MethodName() string {
	return m.next.MethodName()
}

func (m *requestMiddleware) Handle(ctx context.Context, h SHC, req *proto.Request) error {
	if err := m.fn(ctx, h, req); err != nil {
		return err
	}
	return m.next.Handle(ctx, h, req)
}

func Middleware(fn RequestMiddlewareFunc, next RequestHandler) RequestHandler {
	return &requestMiddleware{next: next, fn: fn}
}
