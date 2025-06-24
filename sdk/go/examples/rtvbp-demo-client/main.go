package main

import (
	"context"
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
	var (
		args, log = initCLI()
		ctx       = context.Background()
		e1, e2    = audio.NewDuplexBuffers()
		done      = make(chan error, 1)
	)

	handler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{
			Audio: func() (io.ReadWriter, error) {
				return e2, nil
			},
			BeginHandler: func(ctx context.Context, h rtvbp.HandlerCtx) error {
				// TODO: session.updated event
				_ = h.Notify(ctx, &protov1.SessionUpdatedEvent{
					Audio: &protov1.AudioConfig{
						Channels:   1,
						Format:     "pcm16",
						SampleRate: int(args.sampleRate),
					},
				})

				// emit some event
				/*_ = h.Notify(ctx, &protov1.DummyEvent{
					Text: "hello from client",
				})*/

				// example request
				/*moved, err := h.Request(ctx, &protov1.ApplicationMoveRequest{})
				if err != nil {
					return err
				}
				h.Log().Info("application move response", "response", moved)
				*/

				return nil
			},
		},
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.HandlerCtx, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
			// TODO: log
			return &protov1.ApplicationMoveResponse{}, nil
		}),
	)

	// create and run the session
	log.Info("starting client", slog.String("url", args.url))
	sess := rtvbp.NewSession(
		ws.Client(args.config()),
		handler,
	)
	go func() {
		err := sess.Run(ctx)
		if err != nil {
			done <- err
		}
	}()

	if args.audio {
		go func() {
			err := pipeLocalAudio(
				ctx,
				audio.NewNonBlockingReader(e1),
				e1,
				args.sampleRate,
			)
			if err != nil {
				slog.Error("audio stream failed", slog.Any("err", err))
			}
		}()
	} else {
		// TODO: consume channels
	}

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)

	select {
	case <-ctx.Done():
		_ = sess.Close(5 * time.Second)
	case <-sig:
		_ = sess.Close(5 * time.Second)
	case err := <-done:
		if err != nil {
			log.Error("session failed", slog.Any("err", err))
		}
		_ = sess.Close(5 * time.Second)
	}

	println("DONE")

}
