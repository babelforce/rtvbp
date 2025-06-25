package ws

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/stretchr/testify/require"
	"log/slog"
	"testing"
	"time"
)

func TestClientServer(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelInfo)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	srv := NewServer(ServerConfig{
		Addr: "127.0.0.1:0",
	})
	require.NoError(t, srv.Run(ctx))

	// Connect client transport
	clientTransport, err := Connect(ctx, ClientConfig{Dial: DialConfig{URL: fmt.Sprintf("ws://127.0.0.1:%d", srv.Port())}})
	require.NoError(t, err)

	client := rtvbp.NewSession(clientTransport, rtvbp.SessionConfig{}, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	go client.Run(ctx)
	<-time.After(1 * time.Second)
	require.NoError(t, client.CloseTimeout(5*time.Second))
	require.NoError(t, client.CloseTimeout(5*time.Second))

	// CloseTimeout transport
	closeCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	require.NoError(t, clientTransport.Close(closeCtx))
}
