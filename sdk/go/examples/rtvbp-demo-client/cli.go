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
	url            string // url is the URL to connect to
	logLevel       string // logLevel is the log level user for the client application
	audio          bool   // audio defines if audio is enabled or not
	proxyToken     string // proxyToken
	proxyURL       string
	authToken      string
	sampleRate     int
	audioLatencyMs int
	phone          bool
}

func (a *cliArgs) config() ws.ClientConfig {
	return ws.ClientConfig{
		SampleRate: a.sampleRate,
		Dial: ws.DialConfig{
			URL:            a.connectURL(),
			ConnectTimeout: 5 * time.Second,
			Headers:        a.httpHeader(),
		},
	}
}

func (a *cliArgs) connectURL() string {
	if a.proxyURL != "" {
		return a.proxyURL
	}
	return a.url
}

func (a *cliArgs) httpHeader() http.Header {
	headers := http.Header{}
	if a.authToken != "" {
		headers.Set("authorization", "Bearer "+a.authToken)
	}

	if a.proxyURL != "" {
		if a.proxyToken != "" {
			headers.Set("x-proxy-token", a.proxyToken)
		}
		headers.Set("x-proxy-url", a.url)
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
		url:            "ws://localhost:8080/ws",
		logLevel:       "info",
		audio:          true,
		proxyToken:     "",
		authToken:      "",
		sampleRate:     24_000,
		audioLatencyMs: 50,
	}
	flag.StringVar(&args.url, "url", args.url, "websocket url")
	flag.StringVar(&args.logLevel, "log-level", args.logLevel, "log level")
	flag.StringVar(&args.authToken, "auth-token", args.authToken, "auth token used as Bearer token in Authorization header")
	flag.StringVar(&args.proxyToken, "proxy-token", args.proxyToken, "set header for rtvbp proxy (x-proxy-token)")
	flag.StringVar(&args.proxyURL, "proxy-url", args.proxyURL, "set proxy url for websocket proxy")
	flag.IntVar(&args.sampleRate, "sample-rate", args.sampleRate, "sample rate to send out")
	flag.IntVar(&args.audioLatencyMs, "audio-latency", args.audioLatencyMs, "audio latency in ms")
	flag.BoolVar(&args.audio, "audio", args.audio, "enable audio")
	flag.BoolVar(&args.phone, "phone", args.phone, "set 8khz sample rate and enable audio")
	flag.Parse()

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: args.LogLevel(),
	})))

	log := slog.Default()

	return &args, log
}
