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

type dummyPhoneSystem struct {
	log *slog.Logger
}

func (d *dummyPhoneSystem) Hangup(ctx context.Context) error {
	d.log.Info("hangup")
	return nil
}

func (d *dummyPhoneSystem) Move(ctx context.Context, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
	d.log.Info("move", slog.Any("req", req))
	return &protov1.ApplicationMoveResponse{}, nil
}

var _ protov1.TelephonyAdapter = &dummyPhoneSystem{}

func main() {
	var (
		args, log                 = initCLI()
		ctx                       = context.Background()
		audioDevice, audioSession = audio.NewDuplexBuffers()
		done                      = make(chan error, 1)
	)

	handler := protov1.Handler(
		&dummyPhoneSystem{log: log.With(slog.String("phone_system", "dummy"))},
		&protov1.Config{
			Metadata: map[string]any{
				"recording_consent": true,
				"call": map[string]any{
					"id":   "1234",
					"from": "+4910002000",
					"to":   "+4910002000",
				},
			},
			Audio: &protov1.AudioConfig{
				Channels:   1,
				Format:     "pcm16",
				SampleRate: int(args.sampleRate),
			},
		},
		func(ctx context.Context, s io.ReadWriter) error {
			audio.DuplexCopy(s, audioSession)
			return nil
		},
	)

	// create and run the session
	log.Info("starting client", slog.String("url", args.url))
	sess := rtvbp.NewSession(
		ws.Client(args.config()),
		handler,
	)

	if args.audio {
		go func() {
			err := pipeLocalAudio(
				ctx,
				audio.NewNonBlockingReader(audioDevice),
				audioDevice,
				args.sampleRate,
			)
			if err != nil {
				slog.Error("audio stream failed", slog.Any("err", err))
			}
		}()
	} else {
		// TODO: consume channels
	}

	go func() {
		done <- sess.Run(ctx)
	}()

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)

	select {
	case <-ctx.Done():
		_ = sess.CloseTimeout(5 * time.Second)
	case <-sig:
		_ = sess.CloseTimeout(5 * time.Second)
	case err := <-done:
		if err != nil {
			log.Error("session failed", slog.Any("err", err))
		}
		_ = sess.CloseTimeout(5 * time.Second)
	}

	println("DONE")

}
