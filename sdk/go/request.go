package rtvbp

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"reflect"
	"time"
)

var (
	ErrRequestTimeout          = errors.New("rtvbp: request timeout")
	ErrRequestFailed           = errors.New("rtvbp: request failed")
	ErrRequestValidationFailed = errors.New("rtvbp: request validation failed")
	ErrSessionClosed           = errors.New("rtvbp: session closed")
	ErrResponseAlreadySent     = errors.New("rtvbp: response already sent")
	ErrNoRequestContext        = errors.New("rtvbp: no inbound request context")
)

type Validation interface {
	Validate() error
}

type NamedRequest interface {
	MethodName() string
}

type Request struct {
	ID         string
	Method     string
	Payload    json.RawMessage
	ReceivedAt time.Time
}

type inboundRequestContextKey struct{}

// InboundRequest returns the semantic request currently being handled.
func InboundRequest(ctx context.Context) (Request, bool) {
	request, ok := ctx.Value(inboundRequestContextKey{}).(Request)
	return request, ok
}

type Response struct {
	Payload json.RawMessage
	Err     *WireError
}

type RemoteError struct {
	WireError WireError
}

func (e *RemoteError) Error() string {
	return fmt.Sprintf("%d: %s", e.WireError.Code, e.WireError.Message)
}

type HandlerError struct {
	WireError WireError
	Cause     error
}

func (e *HandlerError) Error() string { return e.WireError.Message }
func (e *HandlerError) Unwrap() error { return e.Cause }

func BadRequest(cause error) error {
	return &HandlerError{
		WireError: WireError{Code: 400, Message: cause.Error()},
		Cause:     cause,
	}
}

func NotImplemented(message string) error {
	return &HandlerError{WireError: WireError{Code: 501, Message: message}}
}

type RequestHandler interface {
	MethodName() string
	Handle(ctx context.Context, handler SHC, request Request) error
}

type typedRequestHandler[REQ NamedRequest, RES any] struct {
	name       string
	h          func(context.Context, SHC, REQ) (RES, error)
	closeAfter bool
}

func (h *typedRequestHandler[REQ, RES]) MethodName() string { return h.name }

func (h *typedRequestHandler[REQ, RES]) Handle(ctx context.Context, handler SHC, request Request) error {
	var params REQ
	payload := request.Payload
	if len(payload) == 0 {
		payload = json.RawMessage("{}")
	}
	if err := json.Unmarshal(payload, &params); err != nil {
		return BadRequest(fmt.Errorf("decode %s request: %w", request.Method, err))
	}
	if validation, ok := any(params).(Validation); ok && !isNil(validation) {
		if err := validation.Validate(); err != nil {
			return BadRequest(err)
		}
	}

	ctx = context.WithValue(ctx, inboundRequestContextKey{}, request)
	result, err := h.h(ctx, handler, params)
	if err != nil {
		return err
	}
	if responseDeferred(handler) || responseSent(handler) {
		return nil
	}
	if validation, ok := any(result).(Validation); ok && !isNil(validation) {
		if err := validation.Validate(); err != nil {
			return fmt.Errorf("validate response for %s: %w", request.Method, err)
		}
	}
	payload, err = marshalPayload(result)
	if err != nil {
		return fmt.Errorf("encode response for %s: %w", request.Method, err)
	}
	response := Response{Payload: payload}
	if h.closeAfter {
		return handler.RespondThenClose(ctx, response)
	}
	return handler.Respond(ctx, response)
}

func HandleRequest[REQ NamedRequest, RES any](handler func(context.Context, SHC, REQ) (RES, error)) RequestHandler {
	var zero REQ
	return &typedRequestHandler[REQ, RES]{name: zero.MethodName(), h: handler}
}

// HandleTerminalRequest adapts a typed request handler whose successful response
// is flushed before the session begins graceful shutdown.
func HandleTerminalRequest[REQ NamedRequest, RES any](handler func(context.Context, SHC, REQ) (RES, error)) RequestHandler {
	var zero REQ
	return &typedRequestHandler[REQ, RES]{name: zero.MethodName(), h: handler, closeAfter: true}
}

func HandleWithError[REQ NamedRequest](wireError WireError) RequestHandler {
	return HandleRequest(func(context.Context, SHC, REQ) (any, error) {
		return nil, &HandlerError{WireError: wireError}
	})
}

func marshalPayload(value any) (json.RawMessage, error) {
	if value == nil || isNil(value) {
		return nil, nil
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	return json.RawMessage(encoded), nil
}

func isNil(value any) bool {
	if value == nil {
		return true
	}
	kind := reflect.ValueOf(value).Kind()
	return (kind == reflect.Chan || kind == reflect.Func || kind == reflect.Interface || kind == reflect.Map || kind == reflect.Pointer || kind == reflect.Slice) && reflect.ValueOf(value).IsNil()
}

func wireErrorFrom(err error) *WireError {
	var handlerError *HandlerError
	if errors.As(err, &handlerError) {
		wire := handlerError.WireError
		return &wire
	}
	return &WireError{Code: 500, Message: err.Error()}
}
