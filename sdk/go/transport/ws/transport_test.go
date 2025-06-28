package ws

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/stretchr/testify/require"
	"log/slog"
	"sync/atomic"
	"testing"
	"time"
)

func TestClientServer(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelInfo)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	var (
		srvOnBeginCalled atomic.Bool
		srvOnEndCalled   atomic.Bool
		srvUpdatedEvent  atomic.Bool
	)

	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{
		OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
			srvOnBeginCalled.Store(true)
			return nil
		},
		OnEnd: func(ctx context.Context, h rtvbp.SHC) error {
			srvOnEndCalled.Store(true)
			return nil
		},
	}, rtvbp.HandleEvent(func(ctx context.Context, shc rtvbp.SHC, evt *protov1.SessionUpdatedEvent) error {
		srvUpdatedEvent.Store(true)
		return nil
	}))

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
	require.True(t, srvUpdatedEvent.Load(), "server on event handler not called")

	// --- closing session ---
	require.NoError(t, client.CloseTimeout(5*time.Second))

	require.True(t, srvOnEndCalled.Load())

	// server shutdown
	require.NoError(t, srv.Shutdown(ctx))
}
