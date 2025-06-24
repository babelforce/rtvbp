package main

import (
	"context"
	"flag"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"time"
)

func connectClient(
	ctx context.Context,
	url string,
	headers http.Header,
	handler rtvbp.SessionHandler,
) *rtvbp.Session {

	transport, err := ws.Connect(ctx, ws.ClientConfig{
		Dial: ws.DialConfig{
			URL:            url,
			ConnectTimeout: 5 * time.Second,
			Headers:        headers,
		},
	})
	if err != nil {
		panic(err)
	}

	s := rtvbp.NewSession(
		transport,
		rtvbp.SessionConfig{},
		handler,
	)

	go s.Run(ctx)

	return s
}

type cliArgs struct {
	url        string
	logLevel   string
	audio      bool
	proxyToken string
	authToken  string
}

func parseLevel(levelStr string) (slog.Level, error) {
	var lvl slog.Level
	err := lvl.UnmarshalText([]byte(levelStr))
	return lvl, err
}

func main() {
	args := cliArgs{
		url:        "ws://localhost:8080/ws",
		logLevel:   "info",
		audio:      true,
		proxyToken: "",
		authToken:  "",
	}
	flag.StringVar(&args.url, "url", args.url, "websocket url")
	flag.StringVar(&args.logLevel, "log-level", args.logLevel, "log level")
	flag.StringVar(&args.authToken, "auth-token", args.authToken, "auth token used as Bearer token in Authorization header")
	flag.StringVar(&args.proxyToken, "proxy-token", args.proxyToken, "set header for rtvbp proxy (x-proxy-token)")
	flag.BoolVar(&args.audio, "audio", args.audio, "enable audio")
	flag.Parse()

	ll, err := parseLevel(args.logLevel)
	if err != nil {
		panic(err)
	}
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: ll,
	})))

	log := slog.Default()
	log.Info("starting client", slog.String("url", args.url))

	// outer context with timeout
	ctx := context.Background()

	e1, e2 := audio.NewDuplex(ctx.Done(), 128)

	headers := http.Header{}
	if args.authToken != "" {
		headers.Set("Authorization", "Bearer "+args.authToken)
	}
	if args.proxyToken != "" {
		headers.Set("x-proxy-token", args.proxyToken)
	}

	// start client
	sess := connectClient(
		ctx,
		args.url,
		headers,
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				AudioIO: e2,
				BeginHandler: func(ctx context.Context, h rtvbp.HandlerCtx) error {
					// TODO: session.updated event

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
		))

	if args.audio {
		//cb := rtvbp.NewChanBuf(1024*1024, false)

		go func() {
			err := pipeLocalAudio(
				ctx,
				e1.ToNonBlockingRW(),
				24_000,
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
	case <-sig:
	}

	_ = sess.Close(5 * time.Second)
}
