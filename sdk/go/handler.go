package rtvbp

import (
	"context"
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/babelforce/rtvbp-go/proto"
	"log/slog"
)

type HandlerCtx interface {
	// SessionID retrieves the ID of the current session
	SessionID() string

	// Log returns the sessions Logger
	Log() *slog.Logger

	// Request performs a request execution
	Request(ctx context.Context, req NamedRequest) (*proto.Response, error)
	Notify(ctx context.Context, evt NamedEvent) error
}

type SessionHandler interface {
	AudioIO() audio.AudioIO
	OnBegin(ctx context.Context, h HandlerCtx) error
	OnEnd(ctx context.Context, h HandlerCtx) error
	OnRequest(ctx context.Context, h HandlerCtx, req *proto.Request) (*proto.Response, error)
	OnEvent(ctx context.Context, h HandlerCtx, evt *proto.Event) error
}

type defaultSessionHandler struct {
	eventHandlers   map[string]EventHandler
	requestHandlers map[string]RequestHandler
	onEnd           func(ctx context.Context, h HandlerCtx) error
	onBegin         func(ctx context.Context, h HandlerCtx) error
	audioIO         audio.AudioIO
}

func (d *defaultSessionHandler) AudioIO() audio.AudioIO {
	return d.audioIO
}

func (d *defaultSessionHandler) OnBegin(ctx context.Context, hc HandlerCtx) error {
	if d.onBegin != nil {
		return d.onBegin(ctx, hc)
	}
	return nil
}

func (d *defaultSessionHandler) OnEnd(ctx context.Context, hc HandlerCtx) error {
	if d.onEnd != nil {
		return d.onEnd(ctx, hc)
	}
	return nil
}

func (d *defaultSessionHandler) OnRequest(ctx context.Context, hc HandlerCtx, req *proto.Request) (*proto.Response, error) {
	hdl, ok := d.requestHandlers[req.Method]
	if !ok {
		// TODO: FAIL with 501 - not implemented
		return nil, nil
	}

	return hdl.Handle(ctx, hc, req)
}

func (d *defaultSessionHandler) OnEvent(ctx context.Context, hc HandlerCtx, evt *proto.Event) error {
	hdl, ok := d.eventHandlers[evt.Event]
	if !ok {
		return nil
	}

	return hdl.Handle(ctx, hc, evt)
}

type HandlerConfig struct {
	BeginHandler func(ctx context.Context, h HandlerCtx) error
	EndHandler   func(ctx context.Context, h HandlerCtx) error
	AudioIO      audio.AudioIO
	AudioIn      <-chan []byte
	AudioOut     chan<- []byte
}

// NewHandler creates a new handler
func NewHandler(config HandlerConfig, args ...any) SessionHandler {
	handler := &defaultSessionHandler{
		eventHandlers:   make(map[string]EventHandler),
		requestHandlers: make(map[string]RequestHandler),
		onBegin:         config.BeginHandler,
		onEnd:           config.EndHandler,
		audioIO:         config.AudioIO,
	}

	if handler.audioIO == nil {

	}

	// TODO: handlers from args
	for _, arg := range args {
		switch arg := arg.(type) {
		case EventHandler:
			handler.eventHandlers[arg.EventName()] = arg
		case RequestHandler:
			handler.requestHandlers[arg.MethodName()] = arg
		}
	}

	return handler
}

var _ SessionHandler = &defaultSessionHandler{}
