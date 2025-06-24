package ws

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
	"log/slog"
	"net/http"
	"net/url"
	"time"
)

type ClientConfig struct {
	Dial         DialConfig
	PingInterval time.Duration
}

func (c *ClientConfig) Defaults() {
	if c.PingInterval == 0 {
		c.PingInterval = 10 * time.Second
	}

	c.Dial.Defaults()
}

type DialConfig struct {
	URL            string
	AuthHeaderFunc func(ctx context.Context) (string, error)
	ConnectTimeout time.Duration
	Headers        http.Header
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
	if d.AuthHeaderFunc != nil {
		authToken, err := d.AuthHeaderFunc(ctx)
		if err != nil {
			return nil, nil, err
		}
		if authToken != "" {
			header.Add("Authorization", fmt.Sprintf("Bearer %s", authToken))
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
func (c *ClientConfig) doConnect(ctx context.Context) (*WebsocketTransport, error) {
	c.Defaults()

	logger := slog.Default().With(
		slog.String("transport", "websocket"),
		slog.String("component", "client"),
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
		logger,
	)

	ok := make(chan struct{})
	go func() {
		ok <- struct{}{}
		t.processConnection(ctx)
	}()
	<-ok

	return t, nil
}

func Client(config ClientConfig) func(ctx context.Context) (rtvbp.Transport, error) {
	return func(ctx context.Context) (rtvbp.Transport, error) {
		return config.doConnect(ctx)
	}
}
