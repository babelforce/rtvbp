package rtvbp

import (
	"babelforce.go/ivr/rtvbp/rtvbp-go/proto"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
)

type SessionHandler interface {
	Audio() (io.ReadWriter, error)
	OnBegin(ctx context.Context, h SHC) error
	OnEnd(ctx context.Context, h SHC) error
	OnRequest(ctx context.Context, h SHC, req *proto.Request) error
	OnEvent(ctx context.Context, h SHC, evt *proto.Event) error
}

// SHC - Session Handler Context
type SHC interface {
	// SessionID retrieves the ID of the current session
	SessionID() string

	// Log returns the sessions Logger
	Log() *slog.Logger

	// Request performs a request execution
	Request(ctx context.Context, req NamedRequest) (*proto.Response, error)
	Respond(ctx context.Context, res *proto.Response) error
	Notify(ctx context.Context, evt NamedEvent) error

	Close(ctx context.Context) error
}

type PostResponseHook interface {
	PostResponseHook(ctx context.Context, hc SHC) error
}

type sessionHandlerCtx struct {
	sess *Session
}

func (shc *sessionHandlerCtx) Respond(ctx context.Context, res *proto.Response) error {
	data, err := json.Marshal(res)
	if err != nil {
		return fmt.Errorf("marshal response: %w", err)
	}

	if err := shc.sess.writeMsgData(ctx, data); err != nil {
		return fmt.Errorf("write response: %w", err)
	}

	return nil
}

func (shc *sessionHandlerCtx) Close(ctx context.Context) error {
	return shc.sess.CloseContext(ctx)
}

func (shc *sessionHandlerCtx) SessionID() string {
	return shc.sess.id
}

func (shc *sessionHandlerCtx) Log() *slog.Logger {
	return shc.sess.logger
}

func (shc *sessionHandlerCtx) Request(ctx context.Context, req NamedRequest) (*proto.Response, error) {
	return shc.sess.Request(ctx, req)
}

func (shc *sessionHandlerCtx) Notify(ctx context.Context, evt NamedEvent) error {
	return shc.sess.Notify(ctx, evt)
}

var _ SHC = &sessionHandlerCtx{}

type defaultSessionHandler struct {
	eventHandlers   map[string]EventHandler
	requestHandlers map[string]RequestHandler
	onEnd           func(ctx context.Context, h SHC) error
	onBegin         func(ctx context.Context, h SHC) error
	audioFactory    func() (io.ReadWriter, error)
}

func (d *defaultSessionHandler) Audio() (io.ReadWriter, error) {
	if d.audioFactory == nil {
		return nil, fmt.Errorf("audio factory not set")
	}
	return d.audioFactory()
}

func (d *defaultSessionHandler) OnBegin(ctx context.Context, hc SHC) error {
	if d.onBegin != nil {
		return d.onBegin(ctx, hc)
	}
	return nil
}

func (d *defaultSessionHandler) OnEnd(ctx context.Context, hc SHC) error {
	if d.onEnd != nil {
		return d.onEnd(ctx, hc)
	}
	return nil
}

func (d *defaultSessionHandler) OnRequest(ctx context.Context, hc SHC, req *proto.Request) error {
	hdl, ok := d.requestHandlers[req.Method]
	if !ok {
		return hc.Respond(ctx, req.NotOk(proto.NewError(501, fmt.Sprintf("unknown method: %s", req.Method))))
	}

	return hdl.Handle(ctx, hc, req)
}

func (d *defaultSessionHandler) OnEvent(ctx context.Context, hc SHC, evt *proto.Event) error {
	hdl, ok := d.eventHandlers[evt.Event]
	if !ok {
		return nil
	}

	return hdl.Handle(ctx, hc, evt)
}

type HandlerConfig struct {
	BeginHandler func(ctx context.Context, h SHC) error
	EndHandler   func(ctx context.Context, h SHC) error

	// Audio creates audio stream for the session
	Audio func() (io.ReadWriter, error)
}

// NewHandler creates a new handler
func NewHandler(config HandlerConfig, args ...any) SessionHandler {
	handler := &defaultSessionHandler{
		eventHandlers:   make(map[string]EventHandler),
		requestHandlers: make(map[string]RequestHandler),
		onBegin:         config.BeginHandler,
		onEnd:           config.EndHandler,
		audioFactory:    config.Audio,
	}

	// add handlers from args
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
