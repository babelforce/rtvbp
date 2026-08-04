package webrtcws

import (
	"bytes"
	"context"
	"io"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"go.uber.org/goleak"
)

func TestClientAndServerSessionsChooseWebRTCAndExchangeTypedControlAndAudio(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	serverSHC := make(chan rtvbp.SHC, 1)
	clientSHC := make(chan rtvbp.SHC, 1)

	serverHandler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
			if err := handler.OpenAudio(ctx, testAudioFormat()); err != nil {
				return err
			}
			serverSHC <- handler
			return nil
		}},
		v1bridge.NewPingHandler(),
		rtvbp.HandleTerminalRequest(func(context.Context, rtvbp.SHC, *babelforcev1.SessionTerminateRequest) (*babelforcev1.EmptyResponse, error) {
			return &babelforcev1.EmptyResponse{}, nil
		}),
	)
	serverConfig := AddToServer(ws.ServerConfig{Addr: "127.0.0.1:0"}, Config{})
	server := ws.NewServer(serverConfig, serverHandler)
	if err := server.Listen(); err != nil {
		t.Fatalf("listen: %v", err)
	}
	t.Cleanup(func() { _ = server.Shutdown(context.Background()) })

	clientWebSocket := server.GetClientConfig()
	clientWebSocket.Subprotocols = []string{Subprotocol}
	clientHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
		if err := handler.AcceptAudio(ctx); err != nil {
			return err
		}
		clientSHC <- handler
		return nil
	}}, v1bridge.NewPingHandler())
	client := rtvbp.NewSession(
		v1classic.Envelope{},
		Client(ClientConfig{WebSocket: clientWebSocket}),
		rtvbp.WithHandler(clientHandler),
	)
	done := client.Run(ctx)

	var clientContext, serverContext rtvbp.SHC
	select {
	case clientContext = <-clientSHC:
	case err := <-done:
		t.Fatalf("client ended before OnBegin: %v", err)
	case <-ctx.Done():
		t.Fatal(ctx.Err())
	}
	select {
	case serverContext = <-serverSHC:
	case err := <-done:
		t.Fatalf("client ended before server OnBegin: %v", err)
	case <-ctx.Done():
		t.Fatal(ctx.Err())
	}

	request := v1bridge.NewPingRequest()
	response, err := babelforcev1.NewApplicationPeer(client).Ping(ctx, request)
	if err != nil {
		t.Fatalf("typed ping: %v", err)
	}
	if response.T0 != request.T0 {
		t.Fatalf("ping t0 = %d, want %d", response.T0, request.T0)
	}

	assertSessionAudio(t, clientContext.AudioStream(), serverContext.AudioStream(), pcmFrame(1200))
	assertSessionAudio(t, serverContext.AudioStream(), clientContext.AudioStream(), pcmFrame(-2400))

	if _, err := babelforcev1.NewApplicationPeer(client).SessionTerminate(ctx, &babelforcev1.SessionTerminateRequest{Reason: "test complete"}); err != nil {
		t.Fatalf("terminal response was not flushed: %v", err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("client session ended with error: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("client session did not close")
	}
	if err := server.Shutdown(context.Background()); err != nil {
		t.Fatalf("shutdown server: %v", err)
	}
	goleak.VerifyNone(t, goleak.IgnoreCurrent())
}

func assertSessionAudio(t *testing.T, source io.Writer, destination io.Reader, sent []byte) {
	t.Helper()
	if _, err := source.Write(sent); err != nil {
		t.Fatalf("write session audio: %v", err)
	}
	received := make([]byte, len(sent))
	if _, err := io.ReadFull(destination, received); err != nil {
		t.Fatalf("read session audio: %v", err)
	}
	if want := decodePCMU(encodePCMU(sent)); !bytes.Equal(received, want) {
		t.Fatal("session audio did not traverse PCMU WebRTC path")
	}
}

func TestServerShutdownClosesActiveWebRTCSession(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	serverReady := make(chan struct{}, 1)
	clientReady := make(chan struct{}, 1)
	serverHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
		if err := handler.AcceptAudio(ctx); err != nil {
			return err
		}
		serverReady <- struct{}{}
		return nil
	}})
	server := ws.NewServer(AddToServer(ws.ServerConfig{
		Addr:        "127.0.0.1:0",
		AudioFormat: testAudioFormat(),
	}, Config{AudioFormat: testAudioFormat()}), serverHandler)
	if err := server.Listen(); err != nil {
		t.Fatal(err)
	}
	clientConfig := server.GetClientConfig()
	clientConfig.Subprotocols = []string{Subprotocol}
	client := rtvbp.NewSession(
		v1classic.Envelope{},
		Client(ClientConfig{WebSocket: clientConfig}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
			if err := handler.OpenAudio(ctx, testAudioFormat()); err != nil {
				return err
			}
			clientReady <- struct{}{}
			return nil
		}})),
	)
	done := client.Run(ctx)
	for _, ready := range []<-chan struct{}{serverReady, clientReady} {
		select {
		case <-ready:
		case err := <-done:
			t.Fatalf("session ended before ready: %v", err)
		case <-ctx.Done():
			t.Fatal(ctx.Err())
		}
	}
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer shutdownCancel()
	if err := server.Shutdown(shutdownCtx); err != nil {
		t.Fatalf("shutdown active server: %v", err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("client after remote shutdown: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("client did not observe remote WebSocket close")
	}
}

func TestWebRTCClientRejectsPlainOnlyServer(t *testing.T) {
	server := ws.NewServer(ws.ServerConfig{Addr: "127.0.0.1:0"}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	if err := server.Listen(); err != nil {
		t.Fatal(err)
	}
	defer server.Shutdown(context.Background())
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	client := rtvbp.NewSession(
		v1classic.Envelope{},
		Client(ClientConfig{WebSocket: ws.ClientConfig{Dial: ws.DialConfig{URL: server.URL()}}}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	select {
	case err := <-client.Run(ctx):
		if err == nil {
			t.Fatal("WebRTC client connected to a plain-only server")
		}
	case <-ctx.Done():
		t.Fatal("incompatible profile was not rejected promptly")
	}
}
