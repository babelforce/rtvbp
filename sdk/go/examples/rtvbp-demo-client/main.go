package main

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
	audiogo "github.com/codewandler/audio-go"
	"github.com/google/uuid"
	"github.com/gordonklaus/portaudio"
	"io"
	"log/slog"
	"os"
	"os/signal"
	"time"
)

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func main() {
	var (
		args, log = initCLI()
		ctx       = context.Background()
	)

	if args.audio {
		must(portaudio.Initialize())
		defer portaudio.Terminate()
	}

	sr := args.sampleRate
	if args.phone {
		sr = 8_000
	}

	// get audio target
	audioSink := func() io.ReadWriter {
		if args.audio {
			audioDev, err := audiogo.NewDevice(sr, 1)
			if err != nil {
				panic(err)
			}
			return audioDev
		}
		return nil
	}()

	phone := &dummyPhoneSystem{
		done: make(chan struct{}),
		log:  log.With(slog.String("phone_system", "dummy")),
	}

	handler := protov1.NewClientHandler(
		phone,
		&protov1.ClientHandlerConfig{
			Call: protov1.CallInfo{
				ID:        uuid.NewString(),
				SessionID: uuid.NewString(),
				From:      "+4910002000",
				To:        "+4910003000",
			},
			App: protov1.AppInfo{
				ID: "1234",
			},
			Metadata: map[string]any{
				"recording_consent": true,
			},
			SampleRate: sr,
		},
		func(ctx context.Context, h rtvbp.SHC) error {
			lat := 20 * time.Millisecond
			s := int(float64(args.sampleRate) * 2 * lat.Seconds())
			audio.DuplexCopy(h.AudioStream(), s, audioSink, s)

			return nil
		},
	)

	// create and run the session
	log.Info("starting client", slog.Any("url", args.connectURL()))
	log.Debug("config", slog.Any("config", args.config()))
	sess := rtvbp.NewSession(
		ws.Client(args.config()),
		rtvbp.WithHandler(handler),
	)

	if args.hangupAfterSeconds > 0 {
		go func() {
			<-time.After(time.Duration(args.hangupAfterSeconds) * time.Second)
			log.Info("simulating hangup", slog.Int("hangup_after_seconds", args.hangupAfterSeconds))
			if err := handler.OnHangup(ctx, sess); err != nil {
				log.Error("failed to simulate hangup", slog.Any("err", err))
			}
			phone.done <- struct{}{}
		}()
	}

	sessDoneCh := sess.Run(ctx)

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)

	select {
	case <-ctx.Done():
		_ = sess.CloseWithTimeout(5 * time.Second)
	case <-sig:
		_ = sess.CloseWithTimeout(5 * time.Second)
	case <-phone.done:
		log.Info("phone system terminated")
		_ = sess.CloseWithTimeout(5 * time.Second)
	case err := <-sessDoneCh:
		if err != nil {
			log.Error("session failed", slog.Any("err", err))
		}
		_ = sess.CloseWithTimeout(5 * time.Second)
	}
}
