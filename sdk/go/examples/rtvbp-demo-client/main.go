package main

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"github.com/codewandler/audio-go"
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

func copyAudio(
	a io.ReadWriter,
	b io.ReadWriter,
	bufSize int,
) error {
	buf := make([]byte, bufSize)
	for {
		n, err := a.Read(buf)
		if err != nil {
			return err
		}
		n, err = b.Write(buf[:n])
		if err != nil {
			return err
		}
	}
}

func streamAudio(
	bufSize int,
	transportAudio io.ReadWriter,
	deviceAudio io.ReadWriter,
) {
	go copyAudio(transportAudio, deviceAudio, bufSize)
	go copyAudio(deviceAudio, transportAudio, bufSize)
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
			audioDev, err := audio.NewAudioIO(audio.Config{
				CaptureSampleRate: sr,
				PlaySampleRate:    sr,
			})
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
		&protov1.HandlerConfig{
			Metadata: map[string]any{
				"recording_consent": true,
				"application": map[string]any{
					"id": "1234",
				},
				"call": map[string]any{
					"id":   "1234",
					"from": "+4910002000",
					"to":   "+4910002000",
				},
			},
			Audio: &protov1.AudioConfig{
				Channels:   1,
				Format:     "pcm16",
				SampleRate: sr,
			},
		},
		func(ctx context.Context, h rtvbp.SHC) error {
			streamAudio(args.chunkSize(), h.AudioStream(), audioSink)
			return nil
		},
	)

	// create and run the session
	log.Info("starting client", slog.String("url", args.url))
	sess := rtvbp.NewSession(
		ws.Client(args.config()),
		handler,
	)

	sessDoneCh := sess.Run(ctx)

	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt)

	select {
	case <-ctx.Done():
		_ = sess.CloseWithTimeout(5 * time.Second)
	case <-sig:
		_ = sess.CloseWithTimeout(5 * time.Second)
	case <-phone.done:
		println("hangup")
		_ = sess.CloseWithTimeout(5 * time.Second)
	case err := <-sessDoneCh:
		if err != nil {
			log.Error("session failed", slog.Any("err", err))
		}
		_ = sess.CloseWithTimeout(5 * time.Second)
	}
}
