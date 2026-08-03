package rtvbp

import "context"

type RequestMiddlewareFunc func(ctx context.Context, handler SHC, request Request) error

type requestMiddleware struct {
	next RequestHandler
	fn   RequestMiddlewareFunc
}

func (m *requestMiddleware) MethodName() string { return m.next.MethodName() }

func (m *requestMiddleware) Handle(ctx context.Context, handler SHC, request Request) error {
	if err := m.fn(ctx, handler, request); err != nil {
		return err
	}
	return m.next.Handle(ctx, handler, request)
}

func Middleware(middleware RequestMiddlewareFunc, next RequestHandler) RequestHandler {
	return &requestMiddleware{next: next, fn: middleware}
}
