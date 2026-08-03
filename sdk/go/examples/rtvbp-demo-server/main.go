package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/audio"
	"github.com/babelforce/rtvbp/sdk/go/proto/protov1"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	audiogo "github.com/codewandler/audio-go"
	"github.com/gordonklaus/portaudio"
)

type serverCLI struct {
	moveAfterSeconds      int
	hangupAfterSeconds    int
	terminateAfterSeconds int
	debug                 bool
	logLevel              string
	audio                 string
	audioSampleRate       int
}

func (s *serverCLI) level() slog.Level {
	switch s.logLevel {
	case "debug":
		return slog.LevelDebug
	case "error":
		return slog.LevelError
	case "warn":
		return slog.LevelWarn
	case "info":
		return slog.LevelInfo
	default:
		return slog.LevelInfo
	}
}

func main() {

	args := serverCLI{
		moveAfterSeconds:      0,
		hangupAfterSeconds:    0,
		terminateAfterSeconds: 0,
		debug:                 false,
		logLevel:              "info",
		audio:                 "loopback",
		audioSampleRate:       8_000,
	}

	flag.IntVar(&args.moveAfterSeconds, "move", args.moveAfterSeconds, "move application after x")
	flag.IntVar(&args.hangupAfterSeconds, "hangup", args.hangupAfterSeconds, "hangup after x seconds")
	flag.IntVar(&args.terminateAfterSeconds, "terminate", args.terminateAfterSeconds, "terminate after x seconds")
	flag.BoolVar(&args.debug, "debug", args.debug, "transport debug messages")
	flag.StringVar(&args.logLevel, "log-level", args.logLevel, "set log level")
	flag.StringVar(&args.audio, "audio", args.audio, "set audio processing")
	flag.IntVar(&args.audioSampleRate, "audio-sample-rate", args.audioSampleRate, "audio sample rate when audio is set to device")
	flag.Parse()

	slog.SetLogLoggerLevel(args.level())
	slog.Info("starting server", slog.Any("args", args))

	if args.audio == "device" {
		if err := portaudio.Initialize(); err != nil {
			panic(err)
		}
		defer portaudio.Terminate()
	}

	// start server
	srv := ws.NewServer(
		ws.ServerConfig{
			Addr:  "0.0.0.0:8080",
			Debug: args.debug,
		},
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {

					if args.moveAfterSeconds != 0 {
						go func() {
							<-time.After(time.Duration(args.moveAfterSeconds) * time.Second)
							_, _ = h.Request(ctx, &protov1.ApplicationMoveRequest{
								Reason:        "auto move",
								ApplicationID: "1234",
							})
						}()
					}

					if args.terminateAfterSeconds != 0 {
						go func() {
							<-time.After(time.Duration(args.terminateAfterSeconds) * time.Second)
							_, _ = h.Request(ctx, &protov1.SessionTerminateRequest{
								Reason: "auto terminate",
							})
						}()
					}

					if args.hangupAfterSeconds != 0 {
						go func() {
							<-time.After(time.Duration(args.hangupAfterSeconds) * time.Second)
							_, _ = h.Request(ctx, &protov1.CallHangupRequest{
								Reason: "auto hangup",
							})
						}()
					}

					return nil
				},
			},
			rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *protov1.SessionInitializeRequest) (*protov1.SessionInitializeResponse, error) {
				if req.AudioCodecOfferings == nil || len(req.AudioCodecOfferings) == 0 {
					return nil, fmt.Errorf("no audio codec offerings")
				}

				// start audio
				if args.audio == "loopback" {
					lb := audio.NewLoopback()
					audio.DuplexCopy(lb, 3200, hc.AudioStream(), 3200)
				} else if args.audio == "device" {
					audioDev, err := audiogo.NewDevice(args.audioSampleRate, 1)
					if err != nil {
						return nil, fmt.Errorf("failed to setup server audio: %w", err)
					}
					lat := 20 * time.Millisecond
					s := int(float64(args.audioSampleRate) * 2 * lat.Seconds())
					audio.DuplexCopy(hc.AudioStream(), s, audioDev, s)
				} else if args.audio == "file" {
					// TODO:
				}

				return &protov1.SessionInitializeResponse{
					AudioCodec: &req.AudioCodecOfferings[0],
				}, nil
			}),
			rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *protov1.SessionTerminateRequest) (*protov1.EmptyResponse, error) {
				return &protov1.EmptyResponse{}, nil
			}),
			rtvbp.HandleEvent(func(ctx context.Context, shc rtvbp.SHC, evt *protov1.DTMFEvent) error {
				println("DTMF>:", evt.Digit, ":", evt.String())
				return nil
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
