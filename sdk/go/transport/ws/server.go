package ws

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
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

		// if auth function is specified validate here
		if config.AuthHandler != nil {
			if err := config.AuthHandler(r); err != nil {
				http.Error(w, "Unauthorized", http.StatusUnauthorized)
				return
			}
		}

		// upgrade connection
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			log.Error("upgrade failed", slog.Any("err", err))
			return
		}
		log.Debug("websocket upgrade successful")

		sess := rtvbp.NewSession(
			rtvbp.WithTransport(func(ctx context.Context, audio io.ReadWriter) (rtvbp.Transport, error) {
				trans := newTransport(
					conn,
					audio,
					&TransportConfig{
						Logger: log,
					},
				)

				go trans.process(ctx)
				return trans, nil
			}),
			rtvbp.WithHandler(handler),
			rtvbp.WithLogger(log),
		)

		// run session
		ctx, cancel := context.WithCancel(r.Context())
		defer cancel()

		select {
		case <-ctx.Done():
			_ = sess.CloseWithTimeout(5 * time.Second)
			return
		case err := <-sess.Run(ctx):
			if err != nil {
				log.Error("session failed", slog.Any("err", err))
			}

		}
	}
}

type ServerConfig struct {
	Addr        string
	Path        string
	ChunkSize   int
	AuthHandler func(req *http.Request) error
}

func (c *ServerConfig) Defaults() {
	if c.Addr == "" {
		c.Addr = "127.0.0.1:8080"
	}
	if c.Path == "" {
		c.Path = "/"
	}
	if c.ChunkSize == 0 {
		c.ChunkSize = 160
	}
}

type Server struct {
	logger   *slog.Logger
	config   ServerConfig
	addr     *net.TCPAddr
	http     *http.Server
	listener net.Listener
}

func (s *Server) Shutdown(ctx context.Context) (err error) {
	err = s.http.Shutdown(ctx)
	s.logger.Info("shutdown complete", slog.Any("err", err))
	return err
}

func (s *Server) URL() string {
	return fmt.Sprintf("ws://%s:%d%s", s.addr.IP, s.addr.Port, s.config.Path)
}

func (s *Server) GetClientConfig() ClientConfig {
	return ClientConfig{
		Dial: DialConfig{
			URL: s.URL(),
		},
		SampleRate: 8000,
	}
}

func (s *Server) NewClientSession(handler rtvbp.SessionHandler) *rtvbp.Session {
	return rtvbp.NewSession(
		Client(s.GetClientConfig()),
		rtvbp.WithHandler(handler),
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
