package main

import (
	"context"
	"io"
	"log/slog"
	"os"
	"os/signal"
	"rtvbp_demo_client/dummyphone"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/audio"
	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	audiogo "github.com/codewandler/audio-go"
	"github.com/google/uuid"
	"github.com/gordonklaus/portaudio"
)

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func main() {
	var (
		args, log = initCLI()
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

	// init phone system
	phone, ctx := dummyphone.New(
		log.With(slog.String("phone_system", "dummy")),
	)

	handler := v1bridge.NewVoiceHandler(
		phone,
		v1bridge.HandlerConfig{
			Call: v1.CallInfo{
				ID:        uuid.NewString(),
				SessionID: uuid.NewString(),
				From:      "+4910002000",
				To:        "+4910003000",
			},
			Application: v1.AppInfo{
				ID: "1234",
			},
			Metadata: map[string]any{
				"recording_consent": true,
			},
			AudioFormat: rtvbp.MediaFormat{
				Encoding:   "L16",
				SampleRate: sr,
				BitDepth:   16,
				Channels:   1,
				PTime:      v1bridge.DefaultPTime,
			},
		},
		func(ctx context.Context, h rtvbp.SHC) error {
			s, err := h.AudioStream().Format().FrameBytes()
			if err != nil {
				return err
			}
			audio.DuplexCopy(h.AudioStream(), s, audioSink, s)

			return nil
		},
	)

	// create and run the session
	log.Info("starting client", slog.Any("url", args.connectURL()))
	log.Debug("config", slog.Any("config", args.config(sr)))
	sess := rtvbp.NewSession(
		v1classic.Envelope{},
		ws.Client(args.config(sr)),
		rtvbp.WithHandler(handler),
		rtvbp.WithRequestTimeout(2*time.Second),
		rtvbp.WithDebug(args.debug),
		rtvbp.WithStreamObserver(handler.Observe(ctx, 5*time.Second)),
	)

	if args.hangupAfterSeconds > 0 {
		go func() {
			<-time.After(time.Duration(args.hangupAfterSeconds) * time.Second)
			log.Info("simulating hangup", slog.Int("hangup_after_seconds", args.hangupAfterSeconds))
			_ = phone.EmulateHangup("timeout")
		}()
	}

	if args.dtmf != "" {
		go func() {
			if args.dtmfDelaySeconds > 0 {
				<-time.After(time.Duration(args.dtmfDelaySeconds) * time.Second)
			}
			phone.EmulateDTMF(args.dtmf)
		}()
	}

	ctrlC := make(chan os.Signal, 1)
	signal.Notify(ctrlC, os.Interrupt)

	sessDoneCh := sess.Run(ctx)

	for {
		select {
		case <-ctrlC:
			_ = phone.EmulateHangup("ctrl-c")
		case err := <-sessDoneCh:
			if err != nil {
				log.Error("session ended with error", slog.Any("err", err))
			}
			log.Info("terminated")
			os.Exit(0)
		}
	}
}
