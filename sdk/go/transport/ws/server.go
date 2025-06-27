package ws

import (
	"context"
	"errors"
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
	"log/slog"
	"net"
	"net/http"
)

func serverUpgradeHandler(srv *Server) func(http.ResponseWriter, *http.Request) {
	var upgrader = websocket.Upgrader{}

	return func(w http.ResponseWriter, r *http.Request) {
		logger := srv.logger.With(
			slog.String("remote_addr", r.RemoteAddr),
			slog.String("path", r.URL.Path),
		)

		logger.Debug("handling websocket upgrade", slog.Any("request", r))

		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			logger.Error("upgrade failed", slog.Any("err", err))
			return
		}

		defer conn.Close()

		t := newTransport(conn, logger)
		srv.c <- t
		t.processConnection(r.Context())
	}
}

type ServerConfig struct {
	Addr string
	Path string
}

type Server struct {
	logger   *slog.Logger
	config   ServerConfig
	c        chan rtvbp.Transport
	port     int
	http     *http.Server
	listener net.Listener
}

func (s *Server) Channel() <-chan rtvbp.Transport {
	return s.c
}

func (s *Server) Shutdown(ctx context.Context) error {
	return s.http.Shutdown(ctx)
}

func (s *Server) Port() int {
	return s.port
}

func (s *Server) Run(ctx context.Context) error {
	var err error
	s.listener, err = net.Listen("tcp", s.config.Addr)
	if err != nil {
		return err
	}
	if tcpAddr, ok := s.listener.Addr().(*net.TCPAddr); ok {
		s.port = tcpAddr.Port
		s.logger = slog.Default().With(
			slog.String("transport", "websocket"),
			slog.String("component", "server"),
			slog.String("addr", tcpAddr.String()),
		)
	}

	s.logger.Info("listening")

	//
	ready := make(chan struct{})
	serveErr := make(chan error, 1)
	go func() {
		close(ready)
		if err := s.http.Serve(s.listener); err != nil && !errors.Is(err, http.ErrServerClosed) {
			serveErr <- err
		}
	}()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-ready:
		return nil
	case err := <-serveErr:
		return err
	}
}

func NewServer(config ServerConfig) *Server {
	s := &Server{
		logger: slog.Default().With(
			slog.String("transport", "websocket"),
			slog.String("component", "server"),
		),
		config: config,
		c:      make(chan rtvbp.Transport, 1),
	}

	// handler
	mux := http.NewServeMux()
	path := config.Path
	if path == "" {
		path = "/"
	}
	mux.HandleFunc(path, serverUpgradeHandler(s))

	s.http = &http.Server{
		Addr:    s.config.Addr,
		Handler: mux,
	}

	return s
}

var _ rtvbp.Acceptor = &Server{}
