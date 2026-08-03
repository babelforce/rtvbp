package ws

import (
	"context"
	"log/slog"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/stretchr/testify/require"
)

func TestTransport_Close(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	srv := NewServer(ServerConfig{
		Addr:      "127.0.0.1:0",
		ChunkSize: 1000,
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	trans, err := Connect(ctx, srv.GetClientConfig(), nil)
	require.NoError(t, err)
	require.NotNil(t, trans)

	closeCtx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()
	require.NoError(t, trans.Close(closeCtx))

	select {
	case _, ok := <-trans.closeChan:
		require.False(t, ok, "close channel not closed")
	default:
		require.Fail(t, "not working!")
	}
}

func TestTransport_CloseByContext(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	srv := NewServer(ServerConfig{
		Addr:      "127.0.0.1:0",
		ChunkSize: 1000,
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
	defer cancel()

	trans, err := Connect(ctx, srv.GetClientConfig(), nil)
	require.NoError(t, err)
	require.NotNil(t, trans)

	<-ctx.Done()
	<-time.After(100 * time.Millisecond)

	select {
	case <-trans.doneChan:
	default:
		require.Fail(t, "close channel not closed")
	}

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
		Addr:      "127.0.0.1:0",
		ChunkSize: 1000,
	}, handler)

	err := srv.Listen()
	if err != nil {
		return
	}

	// Connect client transport
	client := srv.NewClientSession(rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			return h.Notify(ctx, &protov1.SessionUpdatedEvent{})
		},
	}))
	select {
	case err := <-client.Run(ctx):
		require.NoError(t, err)
	default:
	}

	<-time.After(100 * time.Millisecond)

	require.True(t, srvOnBeginCalled.Load(), "server on begin handler not called")

	// --- closing session ---
	require.NoError(t, client.Close(context.Background(), nil))

	// server shutdown
	require.NoError(t, srv.Shutdown(ctx))
}

func TestServerGoesAway(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	// start server
	srv := NewServer(ServerConfig{
		Addr:      "127.0.0.1:0",
		ChunkSize: 1000,
	}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	require.NoError(t, srv.Listen())

	// connect client
	clientConfig := srv.GetClientConfig()
	clientConfig.PingInterval = 500 * time.Millisecond
	trans, err := Connect(ctx, clientConfig, nil)
	require.NoError(t, err)
	require.NotNil(t, trans)

	// shutdown server
	require.NoError(t, srv.Shutdown(ctx), "server shutdown failed")

	require.NoError(t, trans.Close(context.Background()))
}
