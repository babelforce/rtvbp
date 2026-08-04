package main

import (
	"io"
	"log/slog"
	"reflect"
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

func TestDemoClientSelectsAudioTransportWithoutChangingSessionSetup(t *testing.T) {
	websocketArgs := &cliArgs{url: "ws://127.0.0.1:8080/ws", audioTransport: "websocket"}
	if option, err := websocketArgs.transportOption(8_000); err != nil || option == nil {
		t.Fatalf("WebSocket transport option = %v, %v", option, err)
	}
	webrtcArgs := &cliArgs{
		url:            "ws://127.0.0.1:8080/ws",
		audioTransport: "webrtc",
		iceServers:     "stun:stun.example.test:3478, turns:turn.example.test:5349",
		iceUsername:    "user",
		iceCredential:  "secret",
	}
	if option, err := webrtcArgs.transportOption(8_000); err != nil || option == nil {
		t.Fatalf("WebRTC transport option = %v, %v", option, err)
	}
	wantURLs := []string{"stun:stun.example.test:3478", "turns:turn.example.test:5349"}
	if got := webrtcArgs.pionConfiguration().ICEServers[0].URLs; !reflect.DeepEqual(got, wantURLs) {
		t.Fatalf("ICE URLs = %v, want %v", got, wantURLs)
	}
	if _, err := webrtcArgs.transportOption(24_000); err == nil {
		t.Fatal("WebRTC accepted unsupported 24 kHz demo audio")
	}
	webrtcArgs.audioTransport = "unknown"
	if _, err := webrtcArgs.transportOption(8_000); err == nil {
		t.Fatal("unknown transport accepted")
	}
}
