package main

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	babelforcev1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/catalog/demov1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"go.uber.org/goleak"
)

func TestMain(m *testing.M) {
	goleak.VerifyTestMain(m)
}

func TestBothNegotiatedProfilesCompleteAnExchange(t *testing.T) {
	server := httptest.NewServer(httptestHandler())
	defer server.Close()
	url := "ws" + strings.TrimPrefix(server.URL, "http")

	t.Run("demo", func(t *testing.T) {
		observed := make(chan string, 1)
		handler := rtvbp.NewHandler(
			rtvbp.HandlerConfig{},
			demov1.VoiceEventHandlers(demoVoiceEvents{observed: observed})...,
		)
		session, transport, done := runClient(t, url, []string{demoProfile}, handler)
		if transport.Subprotocol() != demoProfile || transport.WireSubprotocol() != demoProfile {
			t.Fatalf("profile = effective:%q wire:%q", transport.Subprotocol(), transport.WireSubprotocol())
		}
		response, err := demov1.NewApplicationPeer(session).DemoEcho(testContext(t), &demov1.DemoEchoRequest{Message: "hello"})
		if err != nil || response.Message != "hello" {
			t.Fatalf("demo.echo response=%#v error=%v", response, err)
		}
		select {
		case message := <-observed:
			if message != "hello" {
				t.Fatalf("demo.observed = %q", message)
			}
		case <-time.After(2 * time.Second):
			t.Fatal("demo.observed was not received")
		}
		closeClient(t, session, done)
	})

	t.Run("legacy-default-without-header", func(t *testing.T) {
		session, transport, done := runClient(t, url, []string{}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
		if transport.Subprotocol() != ws.DefaultSubprotocol || transport.WireSubprotocol() != "" {
			t.Fatalf("profile = effective:%q wire:%q", transport.Subprotocol(), transport.WireSubprotocol())
		}
		response, err := babelforcev1.NewApplicationPeer(session).Ping(
			testContext(t),
			&babelforcev1.PingRequest{T0: time.Now().UnixMilli()},
		)
		if err != nil || response.T0 == 0 {
			t.Fatalf("ping response=%#v error=%v", response, err)
		}
		closeClient(t, session, done)
	})
}

func httptestHandler() *profileHTTPHandler {
	return &profileHTTPHandler{profiles: profileHandlers()}
}

type profileHTTPHandler struct {
	profiles map[string]rtvbp.SessionHandler
}

func (handler *profileHTTPHandler) ServeHTTP(writer http.ResponseWriter, request *http.Request) {
	serveProfiles(handler.profiles)(writer, request)
}

type demoVoiceEvents struct{ observed chan<- string }

func (events demoVoiceEvents) DemoObserved(_ context.Context, _ rtvbp.SHC, event *demov1.DemoObservedEvent) error {
	events.observed <- event.Message
	return nil
}

func runClient(t *testing.T, url string, subprotocols []string, handler rtvbp.SessionHandler) (*rtvbp.Session, *ws.Transport, <-chan error) {
	t.Helper()
	transport, err := ws.Dial(testContext(t), ws.ClientConfig{
		Dial: ws.DialConfig{URL: url}, Subprotocols: subprotocols,
	})
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	var id atomic.Int64
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithHandler(handler),
		rtvbp.WithIDGenerator(func() string { return fmt.Sprintf("request-%d", id.Add(1)) }),
	)
	done := session.Run(context.Background())
	deadline := time.Now().Add(2 * time.Second)
	for session.State() != rtvbp.SessionStateActive && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if session.State() != rtvbp.SessionStateActive {
		t.Fatalf("session state = %s", session.State())
	}
	return session, transport, done
}

func closeClient(t *testing.T, session *rtvbp.Session, done <-chan error) {
	t.Helper()
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("close: %v", err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("run: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("session did not finish")
	}
}

func testContext(t *testing.T) context.Context {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	t.Cleanup(cancel)
	return ctx
}
