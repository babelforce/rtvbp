package ws

import (
	"context"
	"errors"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
	"log/slog"
	"net"
	"net/http"
	"time"
)

func serverUpgradeHandler(
	config *ServerConfig,
	logger *slog.Logger,
	handler rtvbp.SessionHandler,
) func(http.ResponseWriter, *http.Request) {
	var upgrader = websocket.Upgrader{}

	return func(w http.ResponseWriter, r *http.Request) {
		// init logger
		log := logger.With(
			slog.String("remote_addr", r.RemoteAddr),
			slog.String("path", r.URL.Path),
		)
		log.Debug("handling websocket upgrade", slog.Any("request", r))

		// upgrade connection
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Error("upgrade failed", slog.Any("err", err))
			return
		}
		log.Debug("websocket upgrade successful")

		sess := rtvbp.NewSession(
			func(ctx context.Context) (rtvbp.Transport, error) {
				trans := newTransport(
					conn,
					&TransportConfig{
						ChunkSize: config.ChunkSize,
						Logger:    log,
					},
				)

				go trans.process(ctx)
				return trans, nil
			},
			handler,
		)

		// run session
		ctx, cancel := context.WithCancel(r.Context())
		defer cancel()

		select {
		case <-ctx.Done():
			_ = sess.CloseTimeout(5 * time.Second)
			return
		case err := <-sess.Run(ctx):
			log.Error("session failed", slog.Any("err", err))
		}
	}
}

type ServerConfig struct {
	Addr      string
	Path      string
	ChunkSize int
}

func (c *ServerConfig) Defaults() {
	if c.Addr == "" {
		c.Addr = ":8080"
	}
	if c.Path == "" {
		c.Path = "/"
	}
}

type Server struct {
	logger   *slog.Logger
	config   ServerConfig
	addr     *net.TCPAddr
	http     *http.Server
	listener net.Listener
}

func (s *Server) Shutdown(ctx context.Context) error {
	return s.http.Shutdown(ctx)
}

func (s *Server) GetClientConfig() ClientConfig {
	return ClientConfig{
		Dial: DialConfig{
			URL: fmt.Sprintf("ws://%s:%d%s", s.addr.IP, s.addr.Port, s.config.Path),
		},
	}
}

func (s *Server) NewClientSession(handler rtvbp.SessionHandler) *rtvbp.Session {
	return rtvbp.NewSession(
		Client(s.GetClientConfig()),
		handler,
	)
}

func (s *Server) Listen() error {
	var err error
	s.listener, err = net.Listen("tcp", s.config.Addr)
	if err != nil {
		return err
	}
	if tcpAddr, ok := s.listener.Addr().(*net.TCPAddr); ok {
		s.addr = tcpAddr
		s.logger = s.logger.With(
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
	case <-ready:
		return nil
	case err := <-serveErr:
		return err
	}
}

func NewServer(
	config ServerConfig,
	handler rtvbp.SessionHandler,
) *Server {
	config.Defaults()

	logger := slog.Default().With(
		slog.String("transport", "websocket"),
		slog.String("peer", "server"),
	)

	// handler
	mux := http.NewServeMux()
	path := config.Path
	if path == "" {
		path = "/"
	}
	mux.HandleFunc(path, serverUpgradeHandler(&config, logger, handler))

	return &Server{
		logger: logger,
		config: config,
		http: &http.Server{
			Addr:    config.Addr,
			Handler: mux,
		},
	}
}
