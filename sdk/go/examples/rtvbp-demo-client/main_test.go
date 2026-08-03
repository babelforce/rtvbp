package main

import (
	"io"
	"log/slog"
	"testing"

	"rtvbp_demo_client/dummyphone"

	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	"go.uber.org/goleak"
)

func TestMain(m *testing.M) {
	goleak.VerifyTestMain(m)
}

func TestDemoClientRuntimeWiring(t *testing.T) {
	args := &cliArgs{url: "ws://127.0.0.1:8080/ws"}
	config := args.config(8000)
	if err := config.Validate(); err != nil {
		t.Fatalf("client config: %v", err)
	}
	phone, _ := dummyphone.New(slog.New(slog.NewTextHandler(io.Discard, nil)))
	if handler := v1bridge.NewVoiceHandler(phone, v1bridge.HandlerConfig{}, nil); handler == nil {
		t.Fatal("voice handler is nil")
	}
}
