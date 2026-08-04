package ws

import (
	"context"
	"errors"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/babelforce/rtvbp/sdk/go"
)

func TestServerRejectsFailedAuthorizationBeforeWebSocketUpgrade(t *testing.T) {
	server := &Server{}
	decoratorCalled := false
	config := &ServerConfig{
		AuthHandler: func(*http.Request) error { return errors.New("invalid token") },
		AcceptedTransport: func(context.Context, rtvbp.Envelope, *Transport) (rtvbp.Transport, error) {
			decoratorCalled = true
			return nil, nil
		},
	}
	handler := serverUpgradeHandler(server, config, slog.Default(), nil)
	response := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "http://example.test/rtvbp", nil)

	handler(response, request)

	if response.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want %d", response.Code, http.StatusUnauthorized)
	}
	if server.admissions != 0 {
		t.Fatalf("admissions = %d, want 0 after rejected request", server.admissions)
	}
	if decoratorCalled {
		t.Fatal("accepted transport decorator ran before authorization")
	}
}
