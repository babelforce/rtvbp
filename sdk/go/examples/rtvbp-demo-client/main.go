package main

import (
	"babelforce.go/ivr/rtvbp/rtvbp-go"
	"babelforce.go/ivr/rtvbp/rtvbp-go/audio"
	"babelforce.go/ivr/rtvbp/rtvbp-go/proto/protov1"
	"babelforce.go/ivr/rtvbp/rtvbp-go/transport/ws"
	"bufio"
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"strings"
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
		args, log = initCLI()
		ctx       = context.Background()
		e1, e2    = audio.NewDuplexBuffers()
		done      = make(chan error, 1)
	)

	handler := protov1.Handler(
		&dummyPhoneSystem{log: log.With(slog.String("phone_system", "dummy"))},
		e2,
		&protov1.AudioConfig{
			Channels:   1,
			Format:     "pcm16",
			SampleRate: int(args.sampleRate),
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

	go func() {
		done <- sess.Run(ctx)
	}()

	go func() {
		scanner := bufio.NewScanner(os.Stdin)

		for {
			fmt.Print("> ")
			if !scanner.Scan() {
				break
			}

			input := strings.TrimSpace(scanner.Text())

			switch input {
			case "hangup":
				_, _ = sess.Request(ctx, &protov1.SessionTerminateRequest{
					Reason: "hangup",
				})
			default:
				fmt.Println("Unknown command. Type 'help' for a list of commands.")
			}
		}
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
