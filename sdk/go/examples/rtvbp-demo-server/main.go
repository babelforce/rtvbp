package main

import (
	"context"
	"flag"
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

type serverCLI struct {
	moveAfterSeconds      int
	hangupAfterSeconds    int
	terminateAfterSeconds int
}

func main() {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	args := serverCLI{}

	flag.IntVar(&args.moveAfterSeconds, "move", args.moveAfterSeconds, "move application after x")
	flag.IntVar(&args.hangupAfterSeconds, "hangup", args.hangupAfterSeconds, "hangup after x seconds")
	flag.IntVar(&args.terminateAfterSeconds, "terminate", args.terminateAfterSeconds, "terminate after x seconds")
	flag.Parse()

	slog.Info("starting server", slog.Any("args", args))

	// start server
	srv := ws.NewServer(
		ws.ServerConfig{
			Addr:      "0.0.0.0:8080",
			ChunkSize: 160,
		},
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					fmt.Printf("%s> -- server begin handler ---", h.SessionID())

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
				lb := audio.NewLoopback()
				audio.DuplexCopy(lb, 3200, hc.AudioStream(), 3200)

				return &protov1.SessionInitializeResponse{
					AudioCodec: &req.AudioCodecOfferings[0],
				}, nil
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
