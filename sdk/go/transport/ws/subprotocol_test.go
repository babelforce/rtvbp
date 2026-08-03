package ws

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestDefaultSubprotocolIsOfferedAndSelected(t *testing.T) {
	server, accepted := subprotocolServer(t, nil)
	config := ClientConfig{
		Dial: DialConfig{URL: websocketURL(server.URL)},
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	transport, err := Dial(ctx, config)
	if err != nil {
		t.Fatalf("Dial() error = %v", err)
	}
	t.Cleanup(func() { _ = transport.Close(context.Background()) })
	peer := <-accepted
	t.Cleanup(func() { _ = peer.Close() })

	if got := transport.Subprotocol(); got != DefaultSubprotocol {
		t.Fatalf("Subprotocol() = %q, want %q", got, DefaultSubprotocol)
	}
	if got := transport.WireSubprotocol(); got != DefaultSubprotocol {
		t.Fatalf("WireSubprotocol() = %q, want %q", got, DefaultSubprotocol)
	}
	if got := peer.Subprotocol(); got != DefaultSubprotocol {
		t.Fatalf("server selected subprotocol = %q, want %q", got, DefaultSubprotocol)
	}
}

func TestAbsentSubprotocolUsesDefaultWithoutEchoingIt(t *testing.T) {
	server, accepted := subprotocolServer(t, nil)
	client, response, err := websocket.DefaultDialer.Dial(websocketURL(server.URL), nil)
	if err != nil {
		t.Fatalf("Dial() error = %v", err)
	}
	if response.Header.Get("Sec-WebSocket-Protocol") != "" || client.Subprotocol() != "" {
		t.Fatalf("server echoed an unoffered subprotocol: header=%q selected=%q", response.Header.Get("Sec-WebSocket-Protocol"), client.Subprotocol())
	}
	peer := <-accepted
	t.Cleanup(func() { _ = peer.Close() })

	ctx, cancel := context.WithCancel(context.Background())
	transport, err := NewTransport(ctx, client, nil)
	if err != nil {
		t.Fatalf("NewTransport() error = %v", err)
	}
	t.Cleanup(func() {
		_ = transport.Close(context.Background())
		cancel()
	})
	if got := transport.WireSubprotocol(); got != "" {
		t.Fatalf("WireSubprotocol() = %q, want empty", got)
	}
	if got := transport.Subprotocol(); got != DefaultSubprotocol {
		t.Fatalf("effective Subprotocol() = %q, want %q", got, DefaultSubprotocol)
	}
	if got := peer.Subprotocol(); got != "" {
		t.Fatalf("server selected subprotocol = %q, want empty", got)
	}
}

func TestExplicitUnsupportedSubprotocolIsRejected(t *testing.T) {
	server, _ := subprotocolServer(t, []string{DefaultSubprotocol})
	dialer := *websocket.DefaultDialer
	dialer.Subprotocols = []string{"future.v9"}
	conn, response, err := dialer.Dial(websocketURL(server.URL), nil)
	if conn != nil {
		_ = conn.Close()
	}
	if err == nil {
		t.Fatal("unsupported subprotocol was accepted")
	}
	if response == nil || response.StatusCode != http.StatusBadRequest {
		status := 0
		if response != nil {
			status = response.StatusCode
		}
		t.Fatalf("response status = %d, want %d", status, http.StatusBadRequest)
	}
}

func TestClientSubprotocolPreferenceFallsBackToSupportedProfile(t *testing.T) {
	server, accepted := subprotocolServer(t, []string{DefaultSubprotocol})
	config := ClientConfig{
		Dial:         DialConfig{URL: websocketURL(server.URL)},
		Subprotocols: []string{"future.v9", DefaultSubprotocol},
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	transport, err := Dial(ctx, config)
	if err != nil {
		t.Fatalf("Dial() error = %v", err)
	}
	t.Cleanup(func() { _ = transport.Close(context.Background()) })
	peer := <-accepted
	t.Cleanup(func() { _ = peer.Close() })

	if got := transport.Subprotocol(); got != DefaultSubprotocol {
		t.Fatalf("Subprotocol() = %q, want %q", got, DefaultSubprotocol)
	}
}

func subprotocolServer(t *testing.T, supported []string) (*httptest.Server, <-chan *websocket.Conn) {
	t.Helper()
	accepted := make(chan *websocket.Conn, 1)
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		conn, err := upgradeWebSocket(writer, request, supported)
		if err == nil {
			accepted <- conn
		}
	}))
	t.Cleanup(server.Close)
	return server, accepted
}

func websocketURL(url string) string {
	return "ws" + strings.TrimPrefix(url, "http")
}
