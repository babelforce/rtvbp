package ws

import (
	"context"
	"fmt"
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

func Dial(ctx context.Context, dial DialConfig) (*websocket.Conn, *http.Response, error) {
	dial.Defaults()

	u, err := url.Parse(dial.URL)
	if err != nil {
		return nil, nil, err
	}

	var header = http.Header{}
	if dial.AuthHeaderFunc != nil {
		authToken, err := dial.AuthHeaderFunc(ctx)
		if err != nil {
			return nil, nil, err
		}
		if authToken != "" {
			header.Add("Authorization", fmt.Sprintf("Bearer %s", authToken))
		}
	}
	for k, v := range dial.Headers {
		for _, vv := range v {
			header.Add(k, vv)
		}
	}

	if dial.ConnectTimeout == 0 {
		dial.ConnectTimeout = 10 * time.Second
	}

	dialCtx, cancel := context.WithTimeout(ctx, dial.ConnectTimeout)
	defer cancel()
	return websocket.DefaultDialer.DialContext(dialCtx, u.String(), header)
}

// Connect connects to the websocket endpoint
func Connect(ctx context.Context, config ClientConfig) (*WebsocketTransport, error) {
	config.Defaults()

	logger := slog.Default().With(
		slog.String("transport", "websocket"),
		slog.String("component", "client"),
		slog.String("endpoint", config.Dial.URL),
	)

	logger.Debug("Connecting to websocket endpoint", slog.Any("config", config))

	// Websocket upgrade
	conn, resp, err := Dial(ctx, config.Dial)
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
