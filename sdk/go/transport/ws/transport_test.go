package ws

import (
	"context"
	"log/slog"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
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
		srvOnBeginCalled atomic.Bool
	)

	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			srvOnBeginCalled.Store(true)
			return nil
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
			return nil
		},
	}))
	done := client.Run(ctx)
	require.Eventually(t, srvOnBeginCalled.Load, time.Second, 10*time.Millisecond,
		"server on begin handler not called")

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
