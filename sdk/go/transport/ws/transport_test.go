package ws

import (
	"context"
	"log/slog"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/gorilla/websocket"
	"github.com/stretchr/testify/require"
)

func TestTransport_Close(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	trans, err := Dial(ctx, srv.GetClientConfig())
	require.NoError(t, err)
	require.NotNil(t, trans)

	closeCtx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()
	require.NoError(t, trans.Close(closeCtx))

	select {
	case <-trans.done:
	case <-time.After(time.Second):
		require.Fail(t, "transport did not close")
	}
	require.NoError(t, srv.Shutdown(context.Background()))
}

func TestTransport_CloseByContext(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	trans, err := Dial(ctx, srv.GetClientConfig())
	require.NoError(t, err)
	require.NotNil(t, trans)

	select {
	case <-trans.done:
	case <-time.After(2 * time.Second):
		require.Fail(t, "transport did not close after context cancellation")
	}
	require.NoError(t, trans.Close(context.Background()))
	require.NoError(t, srv.Shutdown(context.Background()))
}

func TestClientServer(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelInfo)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	var (
		srvOnBeginCalled  atomic.Bool
		srvOnBeginErr     = make(chan error, 1)
		clientBeginCalled atomic.Bool
		clientBeginErr    = make(chan error, 1)
	)

	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			err := h.OpenAudio(ctx, defaultAudioFormat())
			srvOnBeginErr <- err
			srvOnBeginCalled.Store(true)
			return err
		},
	})

	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
	}, handler)

	err := srv.Listen()
	if err != nil {
		return
	}

	// Connect client transport
	client := srv.NewClientSession(rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			err := h.AcceptAudio(ctx)
			clientBeginErr <- err
			clientBeginCalled.Store(true)
			return err
		},
	}))
	done := client.Run(ctx)
	require.Eventually(t, srvOnBeginCalled.Load, time.Second, 10*time.Millisecond,
		"server on begin handler not called")
	require.NoError(t, <-srvOnBeginErr)
	require.Eventually(t, clientBeginCalled.Load, time.Second, 10*time.Millisecond,
		"client on begin handler not called")
	require.NoError(t, <-clientBeginErr)

	// --- closing session ---
	require.NoError(t, client.Close(context.Background()))
	select {
	case err := <-done:
		require.NoError(t, err)
	case <-time.After(time.Second):
		require.Fail(t, "client session did not stop")
	}

	// server shutdown
	require.NoError(t, srv.Shutdown(ctx))
}

func TestVoiceServerAcceptsPreconfiguredAudioFromApplicationClient(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	format := rtvbp.MediaFormat{
		Encoding:   "L16",
		SampleRate: 16_000,
		BitDepth:   16,
		Channels:   1,
		PTime:      20 * time.Millisecond,
	}
	serverBegin := make(chan error, 1)
	clientBegin := make(chan error, 1)

	srv := NewServer(ServerConfig{
		Addr:        "127.0.0.1:0",
		AudioFormat: format,
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			err := h.AcceptAudio(ctx)
			serverBegin <- err
			return err
		},
	}))
	require.NoError(t, srv.Listen())

	client := srv.NewClientSession(rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			err := h.OpenAudio(ctx, format)
			clientBegin <- err
			return err
		},
	}))
	done := client.Run(ctx)
	require.NoError(t, <-serverBegin)
	require.NoError(t, <-clientBegin)
	require.NoError(t, client.Close(context.Background()))
	require.NoError(t, <-done)
	require.NoError(t, srv.Shutdown(ctx))
}

func TestServerConfigRejectsInvalidOptionalPolicies(t *testing.T) {
	t.Run("audio format", func(t *testing.T) {
		srv := NewServer(ServerConfig{
			Addr:        "127.0.0.1:0",
			AudioFormat: rtvbp.MediaFormat{Encoding: "PCMU"},
		}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
		require.ErrorContains(t, srv.Listen(), "invalid audio format")
	})

	t.Run("keepalive", func(t *testing.T) {
		srv := NewServer(ServerConfig{
			Addr: "127.0.0.1:0",
			KeepalivePolicy: rtvbp.KeepalivePolicy{
				Interval: time.Second,
			},
		}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
		require.ErrorContains(t, srv.Listen(), "invalid keepalive policy")
	})
}

func TestServerKeepalivePolicyClosesSilentPeer(t *testing.T) {
	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
		KeepalivePolicy: rtvbp.KeepalivePolicy{
			Interval:  5 * time.Millisecond,
			Timeout:   10 * time.Millisecond,
			MaxMisses: 1,
		},
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	conn, _, err := websocket.DefaultDialer.Dial(srv.URL(), nil)
	require.NoError(t, err)
	conn.SetPingHandler(func(string) error { return nil })
	require.NoError(t, conn.SetReadDeadline(time.Now().Add(time.Second)))
	_, _, err = conn.ReadMessage()
	require.Error(t, err, "silent peer remained connected despite server keepalive")
	require.NoError(t, conn.Close())
	require.NoError(t, srv.Shutdown(context.Background()))
}

func TestShutdownRejectsSessionPausedAfterUpgrade(t *testing.T) {
	upgraded := make(chan struct{})
	release := make(chan struct{})
	srv := NewServer(ServerConfig{Addr: "127.0.0.1:0"}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	srv.afterUpgrade = func() {
		close(upgraded)
		<-release
	}
	require.NoError(t, srv.Listen())

	type dialResult struct {
		conn *websocket.Conn
		err  error
	}
	dialDone := make(chan dialResult, 1)
	go func() {
		conn, _, err := websocket.DefaultDialer.Dial(srv.URL(), nil)
		dialDone <- dialResult{conn: conn, err: err}
	}()
	<-upgraded
	result := <-dialDone
	require.NoError(t, result.err)
	defer result.conn.Close()

	shutdownCtx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	shutdownDone := make(chan error, 1)
	go func() { shutdownDone <- srv.Shutdown(shutdownCtx) }()
	require.Eventually(t, func() bool {
		srv.mu.Lock()
		defer srv.mu.Unlock()
		return srv.shuttingDown
	}, time.Second, time.Millisecond)
	select {
	case err := <-shutdownDone:
		t.Fatalf("Shutdown() returned before late-session admission completed: %v", err)
	default:
	}

	close(release)
	require.NoError(t, <-shutdownDone)
	require.NoError(t, result.conn.SetReadDeadline(time.Now().Add(time.Second)))
	_, _, err := result.conn.ReadMessage()
	require.Error(t, err, "late upgraded connection remained open")
	srv.mu.Lock()
	require.Empty(t, srv.sessions)
	require.Zero(t, srv.admissions)
	srv.mu.Unlock()
}

func TestServerGoesAway(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	// start server
	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	// connect client
	trans, err := Dial(ctx, srv.GetClientConfig())
	require.NoError(t, err)
	require.NotNil(t, trans)
	require.Eventually(t, func() bool {
		srv.mu.Lock()
		defer srv.mu.Unlock()
		return len(srv.sessions) == 1
	}, time.Second, 10*time.Millisecond, "server did not register the session")

	// shutdown server
	require.NoError(t, srv.Shutdown(ctx), "server shutdown failed")

	require.NoError(t, trans.Close(context.Background()))
}
