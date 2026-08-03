package rtvbp

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"sync/atomic"
)

type SessionHandler interface {
	OnBegin(ctx context.Context, handler SHC) error
	OnRequest(ctx context.Context, handler SHC, request Request) error
	OnEvent(ctx context.Context, handler SHC, event Event) error
}

type HandlerAudio interface {
	io.ReadWriter
	ClearReadBuffer() (int, error)
	Format() MediaFormat
}

type DeferredResponse interface {
	Respond(ctx context.Context, response Response) error
	RespondThenClose(ctx context.Context, response Response) error
}

type SHC interface {
	SessionID() string
	Log() *slog.Logger
	Request(ctx context.Context, request NamedRequest) (Response, error)
	Respond(ctx context.Context, response Response) error
	RespondThenClose(ctx context.Context, response Response) error
	DeferResponse() (DeferredResponse, error)
	Notify(ctx context.Context, event NamedEvent) error
	OpenAudio(ctx context.Context, format MediaFormat) error
	AcceptAudio(ctx context.Context) error
	AudioStream() HandlerAudio
	Close(ctx context.Context) error
	State() SessionState
}

const (
	replyUnclaimed uint32 = iota
	replyDeferred
	replySent
)

type replyState struct {
	status    atomic.Uint32
	requestID string
}

type sessionHandlerCtx struct {
	sess  *Session
	ha    HandlerAudio
	reply *replyState
}

func (h *sessionHandlerCtx) State() SessionState       { return h.sess.State() }
func (h *sessionHandlerCtx) AudioStream() HandlerAudio { return h.ha }
func (h *sessionHandlerCtx) SessionID() string         { return h.sess.id }
func (h *sessionHandlerCtx) Log() *slog.Logger         { return h.sess.logger }

func (h *sessionHandlerCtx) Request(ctx context.Context, request NamedRequest) (Response, error) {
	return h.sess.Request(ctx, request)
}

func (h *sessionHandlerCtx) Notify(ctx context.Context, event NamedEvent) error {
	return h.sess.EventDispatch(ctx, event)
}

func (h *sessionHandlerCtx) OpenAudio(ctx context.Context, format MediaFormat) error {
	return h.sess.OpenAudio(ctx, format)
}

func (h *sessionHandlerCtx) AcceptAudio(ctx context.Context) error {
	return h.sess.AcceptAudio(ctx)
}

func (h *sessionHandlerCtx) Close(ctx context.Context) error { return h.sess.Close(ctx) }

func (h *sessionHandlerCtx) Respond(ctx context.Context, response Response) error {
	return h.respond(ctx, response, false)
}

func (h *sessionHandlerCtx) RespondThenClose(ctx context.Context, response Response) error {
	return h.respond(ctx, response, true)
}

func (h *sessionHandlerCtx) respond(ctx context.Context, response Response, closeAfter bool) error {
	if h.reply == nil {
		return ErrNoRequestContext
	}
	if h.sess.closing.Load() {
		return ErrSessionClosed
	}
	for {
		status := h.reply.status.Load()
		if status == replySent {
			return ErrResponseAlreadySent
		}
		if h.reply.status.CompareAndSwap(status, replySent) {
			break
		}
	}
	if err := h.sess.sendResponse(ctx, h.reply.requestID, response); err != nil {
		h.sess.requestStop(err, true)
		return err
	}
	if closeAfter {
		h.sess.requestStop(nil, false)
	}
	return nil
}

func (h *sessionHandlerCtx) DeferResponse() (DeferredResponse, error) {
	if h.reply == nil {
		return nil, ErrNoRequestContext
	}
	if !h.reply.status.CompareAndSwap(replyUnclaimed, replyDeferred) {
		return nil, ErrResponseAlreadySent
	}
	return &deferredResponse{handler: h}, nil
}

type deferredResponse struct {
	handler *sessionHandlerCtx
}

func (d *deferredResponse) Respond(ctx context.Context, response Response) error {
	return d.handler.Respond(ctx, response)
}

func (d *deferredResponse) RespondThenClose(ctx context.Context, response Response) error {
	return d.handler.RespondThenClose(ctx, response)
}

func responseDeferred(handler SHC) bool {
	context, ok := handler.(*sessionHandlerCtx)
	return ok && context.reply != nil && context.reply.status.Load() == replyDeferred
}

func responseSent(handler SHC) bool {
	context, ok := handler.(*sessionHandlerCtx)
	return ok && context.reply != nil && context.reply.status.Load() == replySent
}

type defaultSessionHandler struct {
	eventHandlers   map[string]EventHandler
	requestHandlers map[string]RequestHandler
	onBegin         func(context.Context, SHC) error
	onUnknownMethod func(context.Context, SHC, Request) error
	onUnknownEvent  func(context.Context, SHC, Event) error
}

func (h *defaultSessionHandler) OnBegin(ctx context.Context, handler SHC) error {
	if h.onBegin != nil {
		return h.onBegin(ctx, handler)
	}
	return nil
}

func (h *defaultSessionHandler) OnRequest(ctx context.Context, handler SHC, request Request) error {
	registered, ok := h.requestHandlers[request.Method]
	if !ok {
		if h.onUnknownMethod != nil {
			return h.onUnknownMethod(ctx, handler, request)
		}
		return NotImplemented(fmt.Sprintf("unknown method: %s", request.Method))
	}
	return registered.Handle(ctx, handler, request)
}

func (h *defaultSessionHandler) OnEvent(ctx context.Context, handler SHC, event Event) error {
	registered, ok := h.eventHandlers[event.Name]
	if !ok {
		if h.onUnknownEvent != nil {
			return h.onUnknownEvent(ctx, handler, event)
		}
		return nil
	}
	return registered.Handle(ctx, handler, event)
}

type HandlerConfig struct {
	OnBegin         func(context.Context, SHC) error
	OnUnknownMethod func(context.Context, SHC, Request) error
	OnUnknownEvent  func(context.Context, SHC, Event) error
}

func NewHandler(config HandlerConfig, handlers ...any) SessionHandler {
	handler := &defaultSessionHandler{
		eventHandlers:   make(map[string]EventHandler),
		requestHandlers: make(map[string]RequestHandler),
		onBegin:         config.OnBegin,
		onUnknownMethod: config.OnUnknownMethod,
		onUnknownEvent:  config.OnUnknownEvent,
	}
	for _, candidate := range handlers {
		switch candidate := candidate.(type) {
		case EventHandler:
			handler.eventHandlers[candidate.EventName()] = candidate
		case RequestHandler:
			handler.requestHandlers[candidate.MethodName()] = candidate
		}
	}
	return handler
}

var _ SHC = (*sessionHandlerCtx)(nil)
var _ DeferredResponse = (*deferredResponse)(nil)
var _ SessionHandler = (*defaultSessionHandler)(nil)
