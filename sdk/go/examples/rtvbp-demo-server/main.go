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
	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
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

type applicationHandler struct {
	args *serverCLI
}

func (handler *applicationHandler) OnBegin(ctx context.Context, shc rtvbp.SHC) error {
	peer := v1.NewVoicePeer(shc)
	if handler.args.moveAfterSeconds != 0 {
		go func() {
			<-time.After(time.Duration(handler.args.moveAfterSeconds) * time.Second)
			_, _ = peer.ApplicationMove(ctx, &v1.ApplicationMoveRequest{
				Reason: "auto move", ApplicationID: "1234",
			})
		}()
	}
	if handler.args.terminateAfterSeconds != 0 {
		go func() {
			<-time.After(time.Duration(handler.args.terminateAfterSeconds) * time.Second)
			// This reverse request intentionally exercises babelforce.v1's frozen 501 rejection.
			_, _ = shc.Request(ctx, &v1.SessionTerminateRequest{Reason: "auto terminate"})
		}()
	}
	if handler.args.hangupAfterSeconds != 0 {
		go func() {
			<-time.After(time.Duration(handler.args.hangupAfterSeconds) * time.Second)
			_, _ = peer.CallHangup(ctx, &v1.CallHangupRequest{Reason: "auto hangup"})
		}()
	}
	return nil
}

func (*applicationHandler) Ping(
	ctx context.Context,
	_ rtvbp.SHC,
	request *v1.PingRequest,
) (*v1.PingResponse, error) {
	inbound, ok := rtvbp.InboundRequest(ctx)
	if !ok {
		return nil, fmt.Errorf("missing inbound request")
	}
	now := time.Now().UnixMilli()
	return &v1.PingResponse{
		T0: request.T0, T1: inbound.ReceivedAt.UnixMilli(), T2: now, OWD: now - request.T0, Data: request.Data,
	}, nil
}

func (handler *applicationHandler) SessionInitialize(
	ctx context.Context,
	shc rtvbp.SHC,
	request *v1.SessionInitializeRequest,
) (*v1.SessionInitializeResponse, error) {
	if len(request.AudioCodecOfferings) == 0 {
		return nil, fmt.Errorf("no audio codec offerings")
	}
	selected := &request.AudioCodecOfferings[0]
	format, err := v1bridge.MediaFormat(selected, v1bridge.DefaultPTime)
	if err != nil {
		return nil, err
	}
	if err := shc.OpenAudio(ctx, format); err != nil {
		return nil, err
	}
	frameBytes, err := format.FrameBytes()
	if err != nil {
		return nil, err
	}
	switch handler.args.audio {
	case "loopback":
		loopback := audio.NewLoopback()
		audio.DuplexCopy(loopback, frameBytes*10, shc.AudioStream(), frameBytes*10)
	case "device":
		audioDevice, err := audiogo.NewDevice(handler.args.audioSampleRate, 1)
		if err != nil {
			return nil, fmt.Errorf("failed to setup server audio: %w", err)
		}
		audio.DuplexCopy(shc.AudioStream(), frameBytes, audioDevice, frameBytes)
	case "file":
		// TODO: add a file-backed audio source.
	}
	return &v1.SessionInitializeResponse{AudioCodec: selected}, nil
}

func (*applicationHandler) SessionTerminate(
	context.Context,
	rtvbp.SHC,
	*v1.SessionTerminateRequest,
) (*v1.EmptyResponse, error) {
	return &v1.EmptyResponse{}, nil
}

func (*applicationHandler) AudioInfo(context.Context, rtvbp.SHC, *v1.AudioInfoEvent) error {
	return nil
}

func (*applicationHandler) CallHangup(context.Context, rtvbp.SHC, *v1.CallHangupEvent) error {
	return nil
}

func (*applicationHandler) Dtmf(_ context.Context, _ rtvbp.SHC, event *v1.DtmfEvent) error {
	slog.Info("DTMF", slog.String("digit", event.Digit), slog.Int("sequence", event.Seq))
	return nil
}

func (*applicationHandler) SessionUpdated(context.Context, rtvbp.SHC, *v1.SessionUpdatedEvent) error {
	return nil
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

	handler := &applicationHandler{args: &args}
	registrations := v1.ApplicationHandlers(handler)
	registrations = append(registrations, v1.ApplicationEventHandlers(handler)...)

	srv := ws.NewServer(
		ws.ServerConfig{
			Addr:  "0.0.0.0:8080",
			Path:  "/ws",
			Debug: args.debug,
		},
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{OnBegin: handler.OnBegin},
			registrations...,
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
