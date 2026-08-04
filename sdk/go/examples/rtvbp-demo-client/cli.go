package main

import (
	"flag"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/webrtcws"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/pion/webrtc/v4"
)

type cliArgs struct {
	url                string // url is the URL to connect to
	logLevel           string // logLevel is the log level user for the client application
	audio              bool   // audio defines if audio is enabled or not
	proxyToken         string // proxyToken
	proxyURL           string
	authToken          string
	authJWT            bool
	sampleRate         int
	phone              bool
	hangupAfterSeconds int
	debug              bool
	dtmf               string // dtmf sequence to send
	dtmfDelaySeconds   int    // dtmf sequence to send after x seconds
	audioTransport     string
	iceServers         string
	iceUsername        string
	iceCredential      string
}

func (a *cliArgs) transportOption(sampleRate int) (rtvbp.Option, error) {
	config := a.config(sampleRate)
	switch a.audioTransport {
	case "", "websocket":
		return ws.Client(config), nil
	case "webrtc":
		if sampleRate != 8_000 {
			return nil, fmt.Errorf("WebRTC audio requires -sample-rate 8000")
		}
		return webrtcws.Client(webrtcws.ClientConfig{
			WebSocket:      config,
			PeerConnection: a.pionConfiguration(),
		}), nil
	default:
		return nil, fmt.Errorf("unknown audio transport %q", a.audioTransport)
	}
}

func (a *cliArgs) pionConfiguration() webrtc.Configuration {
	urls := splitNonEmpty(a.iceServers)
	if len(urls) == 0 {
		return webrtc.Configuration{}
	}
	return webrtc.Configuration{ICEServers: []webrtc.ICEServer{{
		URLs:       urls,
		Username:   a.iceUsername,
		Credential: a.iceCredential,
	}}}
}

func splitNonEmpty(value string) []string {
	var result []string
	for _, item := range strings.Split(value, ",") {
		if trimmed := strings.TrimSpace(item); trimmed != "" {
			result = append(result, trimmed)
		}
	}
	return result
}

func (a *cliArgs) config(sampleRate int) ws.ClientConfig {
	return ws.ClientConfig{
		AudioFormat: rtvbp.MediaFormat{
			Encoding:   "L16",
			SampleRate: sampleRate,
			BitDepth:   16,
			Channels:   1,
			PTime:      20 * time.Millisecond,
		},
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

	if a.authJWT {
		// Generate JWT token
		jwt, err := generateJWT()
		if err != nil {
			panic(fmt.Errorf("JWT generation failed: %w", err))
		}
		headers.Set("authorization", "Bearer "+jwt)
	}

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
		url:                "ws://localhost:8080/ws",
		logLevel:           "info",
		audio:              true,
		proxyToken:         "",
		authToken:          "",
		authJWT:            false,
		sampleRate:         24_000,
		hangupAfterSeconds: 0,
		debug:              false,
		audioTransport:     "websocket",
		iceServers:         os.Getenv("RTVBP_ICE_SERVERS"),
		iceUsername:        os.Getenv("RTVBP_ICE_USERNAME"),
		iceCredential:      os.Getenv("RTVBP_ICE_CREDENTIAL"),
	}
	flag.StringVar(&args.url, "url", args.url, "websocket url")
	flag.StringVar(&args.logLevel, "log-level", args.logLevel, "log level")
	flag.StringVar(&args.authToken, "auth-token", args.authToken, "auth token used as Bearer token in Authorization header")
	flag.BoolVar(&args.authJWT, "auth-jwt", args.authJWT, "use asymmetric JWT auth")
	flag.StringVar(&args.proxyToken, "proxy-token", args.proxyToken, "set header for rtvbp proxy (x-proxy-token)")
	flag.StringVar(&args.proxyURL, "proxy-url", args.proxyURL, "set proxy url for websocket proxy")
	flag.StringVar(&args.dtmf, "dtmf", args.dtmf, "send DTMF sequence")
	flag.IntVar(&args.dtmfDelaySeconds, "dtmf-delay", args.dtmfDelaySeconds, "send DTMF sequence after x seconds")
	flag.IntVar(&args.sampleRate, "sample-rate", args.sampleRate, "sample rate to send out")
	flag.IntVar(&args.hangupAfterSeconds, "hangup", args.hangupAfterSeconds, "hangup after x seconds")
	flag.BoolVar(&args.audio, "audio", args.audio, "enable audio")
	flag.BoolVar(&args.phone, "phone", args.phone, "set 8khz sample rate and enable audio")
	flag.BoolVar(&args.debug, "debug", args.debug, "enable debug logging for transport messages")
	flag.StringVar(&args.audioTransport, "audio-transport", args.audioTransport, "preferred audio transport: websocket or webrtc")
	flag.StringVar(&args.iceServers, "ice-servers", args.iceServers, "comma-separated STUN/TURN URLs for WebRTC")
	flag.StringVar(&args.iceUsername, "ice-username", args.iceUsername, "TURN username (or RTVBP_ICE_USERNAME)")
	flag.StringVar(&args.iceCredential, "ice-credential", args.iceCredential, "TURN credential (or RTVBP_ICE_CREDENTIAL)")
	flag.Parse()

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: args.LogLevel(),
	})))

	log := slog.Default()

	return &args, log
}
