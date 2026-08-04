package main

import (
	"testing"

	"github.com/babelforce/rtvbp/sdk/go"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/transport/webrtcws"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"go.uber.org/goleak"
)

func TestMain(m *testing.M) {
	goleak.VerifyTestMain(m)
}

func TestDemoServerOffersBothBindingsInConfiguredPreferenceOrder(t *testing.T) {
	for _, test := range []struct {
		preferred string
		wantFirst string
	}{
		{preferred: "websocket", wantFirst: ws.DefaultSubprotocol},
		{preferred: "webrtc", wantFirst: webrtcws.Subprotocol},
	} {
		args := &serverCLI{preferredTransport: test.preferred}
		config, err := args.transportConfig()
		if err != nil {
			t.Fatalf("preference %q: %v", test.preferred, err)
		}
		if len(config.Subprotocols) != 2 || config.Subprotocols[0] != test.wantFirst {
			t.Fatalf("preference %q subprotocols = %v", test.preferred, config.Subprotocols)
		}
		if config.AcceptedTransport == nil {
			t.Fatalf("preference %q did not install WebRTC decorator", test.preferred)
		}
	}
	if _, err := (&serverCLI{preferredTransport: "unknown"}).transportConfig(); err == nil {
		t.Fatal("unknown transport preference accepted")
	}
}

func TestDemoServerRuntimeWiring(t *testing.T) {
	handler := &applicationHandler{args: &serverCLI{}}
	registrations := v1.ApplicationHandlers(handler)
	registrations = append(registrations, v1.ApplicationEventHandlers(handler)...)
	if sessionHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: handler.OnBegin}, registrations...); sessionHandler == nil {
		t.Fatal("application session handler is nil")
	}
}
