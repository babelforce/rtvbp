package ws

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"net/url"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/gorilla/websocket"
)

type ClientConfig struct {
	Dial        DialConfig
	AudioFormat rtvbp.MediaFormat
	// Subprotocols lists RTVBP profiles in preference order. Nil defaults to rtvbp.v1;
	// an explicitly empty slice sends no subprotocol header for legacy peers.
	Subprotocols []string
}

func (c *ClientConfig) Validate() error {
	if _, err := c.AudioFormat.FrameBytes(); err != nil {
		return fmt.Errorf("invalid audio format: %w", err)
	}
	if c.Dial.Headers.Get("Sec-WebSocket-Protocol") != "" {
		return errors.New("configure WebSocket subprotocols through ClientConfig.Subprotocols")
	}
	return nil
}

func (c *ClientConfig) Defaults() {
	if c.AudioFormat == (rtvbp.MediaFormat{}) {
		c.AudioFormat = defaultAudioFormat()
	}
	c.Subprotocols = defaultSubprotocols(c.Subprotocols)
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

func (d *DialConfig) doDial(ctx context.Context, subprotocols []string) (*websocket.Conn, *http.Response, error) {
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

	dialer := *websocket.DefaultDialer
	dialer.Subprotocols = append([]string(nil), subprotocols...)
	return dialer.DialContext(dialCtx, u.String(), header)
}

// Dial connects a semantic transport to a WebSocket endpoint.
func Dial(ctx context.Context, c ClientConfig) (*Transport, error) {
	c.Defaults()
	conn, logger, err := dialConnection(ctx, c)
	if err != nil {
		return nil, err
	}
	return NewTransport(ctx, conn, &TransportConfig{Logger: logger, AudioFormat: c.AudioFormat}), nil
}

func dialConnection(ctx context.Context, c ClientConfig) (*websocket.Conn, *slog.Logger, error) {
	c.Defaults()

	if err := c.Validate(); err != nil {
		return nil, nil, err
	}

	logger := slog.Default().With(
		slog.String("transport", "websocket"),
		slog.String("peer", "client"),
		slog.String("endpoint", c.Dial.URL),
	)

	logger.Debug("Connecting to websocket endpoint", slog.Any("config", c))

	// Websocket upgrade
	conn, resp, err := c.Dial.doDial(ctx, c.Subprotocols)
	if err != nil {
		return nil, nil, err
	}
	if resp.StatusCode != http.StatusSwitchingProtocols {
		_ = conn.Close()
		return nil, nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	logger = logger.With(
		slog.String("remote_addr", conn.RemoteAddr().String()),
	)

	logger.Debug("Websocket connection established", slog.Any("response", resp))

	return conn, logger, nil
}

func Client(config ClientConfig) rtvbp.Option {
	return rtvbp.WithTransportFactory(
		func(ctx context.Context, _ rtvbp.Envelope) (rtvbp.Transport, error) {
			resolved := config
			resolved.Defaults()
			conn, logger, err := dialConnection(ctx, resolved)
			if err != nil {
				return nil, err
			}
			// TransportFactory's context bounds construction. Session owns the
			// returned transport lifetime and closes it explicitly during teardown.
			return NewTransport(context.WithoutCancel(ctx), conn, &TransportConfig{
				Logger:      logger,
				AudioFormat: resolved.AudioFormat,
			}), nil
		},
	)
}
