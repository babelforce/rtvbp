package main

import (
	"flag"
	"fmt"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"log/slog"
	"net/http"
	"os"
	"time"
)

type cliArgs struct {
	url             string
	logLevel        string
	audio           bool
	proxyToken      string
	authToken       string
	sampleRate      int
	audioBufferSize int
}

func (a *cliArgs) config() ws.ClientConfig {
	return ws.ClientConfig{
		Dial: ws.DialConfig{
			URL:            a.url,
			ConnectTimeout: 5 * time.Second,
			Headers:        a.httpHeader(),
		},
	}
}

func (a *cliArgs) httpHeader() http.Header {
	headers := http.Header{}
	if a.authToken != "" {
		headers.Set("authorization", "Bearer "+a.authToken)
	}
	if a.proxyToken != "" {
		headers.Set("x-proxy-token", a.proxyToken)
	}
	return headers
}

func (a *cliArgs) LogLevel() slog.Level {
	var lvl slog.Level
	err := lvl.UnmarshalText([]byte(a.logLevel))
	if err != nil {
		panic(fmt.Errorf("invalid log level [%s]: %w", a.logLevel, err))
	}
	return lvl
}

func initCLI() (*cliArgs, *slog.Logger) {
	args := cliArgs{
		url:             "ws://localhost:8080/ws",
		logLevel:        "info",
		audio:           true,
		proxyToken:      "",
		authToken:       "",
		sampleRate:      24_000,
		audioBufferSize: 8_000,
	}
	flag.StringVar(&args.url, "url", args.url, "websocket url")
	flag.StringVar(&args.logLevel, "log-level", args.logLevel, "log level")
	flag.StringVar(&args.authToken, "auth-token", args.authToken, "auth token used as Bearer token in Authorization header")
	flag.StringVar(&args.proxyToken, "proxy-token", args.proxyToken, "set header for rtvbp proxy (x-proxy-token)")
	flag.IntVar(&args.sampleRate, "sample-rate", args.sampleRate, "sample rate to send out")
	flag.IntVar(&args.audioBufferSize, "buffer-size", args.audioBufferSize, "audio streaming buffer size in bytes")
	flag.BoolVar(&args.audio, "audio", args.audio, "enable audio")
	flag.Parse()

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: args.LogLevel(),
	})))

	log := slog.Default()

	return &args, log
}
