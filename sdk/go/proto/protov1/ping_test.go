package protov1

import (
	"context"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
	"github.com/babelforce/rtvbp-go/transport/direct"
	"github.com/stretchr/testify/require"

	"testing"
)

func TestPingHandler(t *testing.T) {
	shc := rtvbp.NewTestingSHC()
	h := NewPingHandler()

	req := proto.NewRequest("ping", NewPingRequest())
	req.SetReceivedAt(time.Now().UnixMilli())

	err := h.Handle(context.Background(), shc, req)
	require.NoError(t, err)
	require.Equal(t, req.ID, shc.Response.Response)
}

func TestPinger(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	var (
		ctx = context.Background()
		h   = rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					go startPinger(ctx, 100*time.Millisecond, h)
					return nil
				},
			},
			NewPingHandler(),
		)
		t1, t2 = direct.NewTestSessions(h, h)
	)

	ctx, cancel := context.WithTimeout(ctx, 1*time.Second)
	defer cancel()

	go t1.Run(ctx)
	go t2.Run(ctx)

	<-ctx.Done()
}
