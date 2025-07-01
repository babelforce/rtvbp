package main

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"log/slog"
	"os"
	"os/signal"
	"time"
)

func main() {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	// start server
	srv := ws.NewServer(
		ws.ServerConfig{
			Addr:      ":8080",
			ChunkSize: 160,
		},
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					fmt.Printf("%s> -- server begin handler ---", h.SessionID())

					// start audio
					lb := audio.NewLoopback()
					audio.DuplexCopy(lb, h.AudioStream())

					// after 10 seconds, use application.move to move the session to a new application
					go func() {
						<-time.After(10 * time.Second)
						_, _ = h.Request(ctx, &protov1.ApplicationMoveRequest{
							ApplicationID: "1234",
						})
					}()
					return nil
				},
			},
			rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.SHC, evt *protov1.SessionUpdatedEvent) error {
				hc.Log().Info("session updated", slog.Any("event", evt))

				if evt.Audio != nil {
					fmt.Printf("[session]\nformat: %s\nsample_rate: %d\n", evt.Audio.Format, evt.Audio.SampleRate)
				}

				// TODO: init resampler ...

				return nil
			}),
			rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *protov1.SessionTerminateRequest) (*protov1.SessionTerminateResponse, error) {
				return &protov1.SessionTerminateResponse{}, nil
			}),
			protov1.NewPingHandler(),
		),
	)

	// run server
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go func() {
		err := srv.Listen()
		if err != nil {
			return
		}
	}()

	// wait for signals
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)
	select {
	case <-sig:
	case <-ctx.Done():
	}

	// shutdown server
	ctx, cancel = context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := srv.Shutdown(ctx); err != nil {
		slog.Error("failed to shutdown server", slog.Any("err", err))
	}

}
