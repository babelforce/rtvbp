package main

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"io"
	"log/slog"
	"os"
	"os/signal"
	"time"
)

func main() {
	slog.SetLogLoggerLevel(slog.LevelInfo)

	// start server
	srv := rtvbp.NewServer(
		ws.NewServer(ws.ServerConfig{
			Addr: ":8080",
		}),
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				Audio: func() (io.ReadWriter, error) {
					return audio.NewLoopback(), nil
				},
			},
			rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.HandlerCtx, evt *protov1.DummyEvent) error {
				return nil
			}),
			rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.HandlerCtx, evt *protov1.SessionUpdatedEvent) error {
				hc.Log().Info("session updated", slog.Any("event", evt))

				if evt.Audio != nil {
					fmt.Printf("[session]\nformat: %s\nsample_rate: %d\n", evt.Audio.Format, evt.Audio.SampleRate)
				}

				// TODO: init resampler ...

				return nil
			}),
		),
	)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-time.After(5 * time.Second):
				slog.Info("server stats", slog.Any("stats", srv.Stats()))
			}
		}
	}()

	go srv.Run(ctx)

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)

	select {
	case <-sig:
	case <-ctx.Done():
	}

	if err := srv.Shutdown(); err != nil {
		slog.Error("failed to shutdown server", slog.Any("err", err))
	}

}
