package rtvbp_test

import (
	"context"
	"errors"
	"testing"

	"github.com/babelforce/rtvbp/sdk/go"
)

func TestDefaultHandlerUnknownMethodReturnsNotImplemented(t *testing.T) {
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{})
	err := handler.OnRequest(
		context.Background(),
		&rtvbp.TestingSHC{},
		rtvbp.Request{ID: "request-1", Method: "unknown.method"},
	)

	var handlerErr *rtvbp.HandlerError
	if !errors.As(err, &handlerErr) {
		t.Fatalf("OnRequest() error = %v, want *rtvbp.HandlerError", err)
	}
	if handlerErr.WireError.Code != 501 {
		t.Fatalf("unknown-method code = %d, want 501", handlerErr.WireError.Code)
	}
	if handlerErr.WireError.Message != "unknown method: unknown.method" {
		t.Fatalf("unknown-method message = %q", handlerErr.WireError.Message)
	}
}

func TestDefaultHandlerUnknownEventIsIgnored(t *testing.T) {
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{})
	if err := handler.OnEvent(
		context.Background(),
		&rtvbp.TestingSHC{},
		rtvbp.Event{ID: "event-1", Name: "unknown.event"},
	); err != nil {
		t.Fatalf("OnEvent() error = %v, want nil", err)
	}
}

func TestDefaultHandlerUnknownHooksOverrideDefaults(t *testing.T) {
	methodErr := errors.New("method hook")
	eventErr := errors.New("event hook")
	var gotMethod string
	var gotEvent string
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnUnknownMethod: func(_ context.Context, _ rtvbp.SHC, request rtvbp.Request) error {
			gotMethod = request.Method
			return methodErr
		},
		OnUnknownEvent: func(_ context.Context, _ rtvbp.SHC, event rtvbp.Event) error {
			gotEvent = event.Name
			return eventErr
		},
	})

	if err := handler.OnRequest(
		context.Background(),
		&rtvbp.TestingSHC{},
		rtvbp.Request{Method: "custom.method"},
	); !errors.Is(err, methodErr) {
		t.Fatalf("OnRequest() error = %v, want method hook error", err)
	}
	if err := handler.OnEvent(
		context.Background(),
		&rtvbp.TestingSHC{},
		rtvbp.Event{Name: "custom.event"},
	); !errors.Is(err, eventErr) {
		t.Fatalf("OnEvent() error = %v, want event hook error", err)
	}
	if gotMethod != "custom.method" || gotEvent != "custom.event" {
		t.Fatalf("hook inputs = method %q, event %q", gotMethod, gotEvent)
	}
}
