package main

import (
	"testing"

	"github.com/babelforce/rtvbp/sdk/go"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"go.uber.org/goleak"
)

func TestMain(m *testing.M) {
	goleak.VerifyTestMain(m)
}

func TestDemoServerRuntimeWiring(t *testing.T) {
	handler := &applicationHandler{args: &serverCLI{}}
	registrations := v1.ApplicationHandlers(handler)
	registrations = append(registrations, v1.ApplicationEventHandlers(handler)...)
	if sessionHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: handler.OnBegin}, registrations...); sessionHandler == nil {
		t.Fatal("application session handler is nil")
	}
}
