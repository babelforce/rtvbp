package ws

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
	"io"
	"log/slog"
	"net/http"
	"net/url"
	"time"
)

type ClientConfig struct {
	Dial         DialConfig
	PingInterval time.Duration
	SampleRate   int
}

func (c *ClientConfig) Validate() error {
	if c.SampleRate <= 0 {
		return fmt.Errorf("invalid sample rate: %d", c.SampleRate)
	}
	return nil
}

func (c *ClientConfig) Defaults() {
	if c.PingInterval == 0 {
		c.PingInterval = 10 * time.Second
	}
	c.Dial.Defaults()
}

// DialConfig configures websocket dial operation
type DialConfig struct {
	URL                     string                                    // URL is the websocket URL to connect to
	AuthorizationHeaderFunc func(ctx context.Context) (string, error) // AuthorizationHeaderFunc is a function which returns content for the Authorization header
	ConnectTimeout          time.Duration                             // ConnectTimeout is the connection timeout applied when connecting to the URL
	Headers                 http.Header                               // Headers are additional headers presented in the Upgrade request
}

func (d *DialConfig) Defaults() {
	if d.ConnectTimeout == 0 {
		d.ConnectTimeout = 10 * time.Second
	}
}

func (d *DialConfig) doDial(ctx context.Context) (*websocket.Conn, *http.Response, error) {
	d.Defaults()

	u, err := url.Parse(d.URL)
	if err != nil {
		return nil, nil, err
	}

	var header = http.Header{}
	header.Add("User-Agent", "babelforce/rtvbp-go")
	if d.AuthorizationHeaderFunc != nil {
		authorizationHeaderValue, err := d.AuthorizationHeaderFunc(ctx)
		if err != nil {
			return nil, nil, err
		}
		if authorizationHeaderValue != "" {
			header.Add("Authorization", authorizationHeaderValue)
		}
	}
	for k, v := range d.Headers {
		for _, vv := range v {
			header.Add(k, vv)
		}
	}

	if d.ConnectTimeout == 0 {
		d.ConnectTimeout = 10 * time.Second
	}

	dialCtx, cancel := context.WithTimeout(ctx, d.ConnectTimeout)
	defer cancel()
	return websocket.DefaultDialer.DialContext(dialCtx, u.String(), header)
}

// Connect connects to the websocket endpoint
func Connect(ctx context.Context, c ClientConfig, audio io.ReadWriter) (*WebsocketTransport, error) {
	c.Defaults()

	if err := c.Validate(); err != nil {
		return nil, err
	}

	logger := slog.Default().With(
		slog.String("transport", "websocket"),
		slog.String("peer", "client"),
		slog.String("endpoint", c.Dial.URL),
	)

	logger.Debug("Connecting to websocket endpoint", slog.Any("config", c))

	// Websocket upgrade
	conn, resp, err := c.Dial.doDial(ctx)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusSwitchingProtocols {
		return nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	logger = logger.With(
		slog.String("remote_addr", conn.RemoteAddr().String()),
	)

	logger.Debug("Websocket connection established", slog.Any("response", resp))

	t := newTransport(
		conn,
		audio,
		&TransportConfig{
			Logger: logger,
		},
	)

	t.debugMessages = true

	ok := make(chan struct{})
	go func() {
		ok <- struct{}{}
		t.process(ctx)
	}()
	<-ok

	return t, nil
}

func Client(config ClientConfig) rtvbp.Option {
	return rtvbp.WithTransport(
		func(ctx context.Context, audio io.ReadWriter) (rtvbp.Transport, error) {
			return Connect(ctx, config, audio)
		},
	)
}
