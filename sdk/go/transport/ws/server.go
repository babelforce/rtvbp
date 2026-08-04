package ws

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"sync"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
)

func serverUpgradeHandler(
	srv *Server,
	config *ServerConfig,
	logger *slog.Logger,
	handler rtvbp.SessionHandler,
) func(http.ResponseWriter, *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		// init logger
		log := logger.With(
			slog.String("remote_addr", r.RemoteAddr),
			slog.String("path", r.URL.Path),
		)
		log.Debug("handling websocket upgrade", slog.Any("request", r))

		endAdmission, admitted := srv.beginAdmission()
		if !admitted {
			http.Error(w, "Server shutting down", http.StatusServiceUnavailable)
			return
		}
		defer endAdmission()

		// if auth function is specified validate here
		if config.AuthHandler != nil {
			if err := config.AuthHandler(r); err != nil {
				log.Warn("authorization failed", slog.Any("err", err))
				http.Error(w, "Unauthorized", http.StatusUnauthorized)
				return
			}
		}

		// upgrade connection
		conn, err := upgradeWebSocket(w, r, config.Subprotocols)
		if err != nil {
			log.Error("upgrade failed", slog.Any("err", err))
			return
		}
		log.Debug("websocket upgrade successful")
		if srv.afterUpgrade != nil {
			srv.afterUpgrade()
		}

		envelope := v1classic.Envelope{}
		sess := rtvbp.NewSession(
			envelope,
			rtvbp.WithTransportFactory(func(ctx context.Context, env rtvbp.Envelope) (rtvbp.Transport, error) {
				// A hijacked HTTP request context is not the WebSocket lifetime;
				// the owning session closes the transport explicitly.
				base, err := NewTransport(context.WithoutCancel(ctx), conn, &TransportConfig{
					Logger:      log,
					AudioFormat: config.AudioFormat,
				})
				if err != nil {
					return nil, err
				}
				if config.AcceptedTransport == nil {
					return base, nil
				}
				transport, err := config.AcceptedTransport(ctx, env, base)
				if err != nil {
					_ = base.Close(context.Background())
					return nil, err
				}
				if transport == nil {
					_ = base.Close(context.Background())
					return nil, errors.New("websocket: accepted transport decorator returned nil")
				}
				return transport, nil
			}),
			rtvbp.WithHandler(handler),
			rtvbp.WithLogger(log),
			rtvbp.WithDebug(srv.config.Debug),
			rtvbp.WithKeepalivePolicy(config.KeepalivePolicy),
		)

		// run session
		// Gorilla hijacks the HTTP connection during upgrade, so the request
		// context is not a reliable WebSocket lifetime. The server registry and
		// Session own teardown from this point onward.
		ctx, cancel := context.WithCancel(context.WithoutCancel(r.Context()))
		defer cancel()

		doneChan := sess.Run(ctx)

		if !srv.addSession(sess) {
			_ = sess.Close(context.Background())
			return
		}
		endAdmission()
		defer srv.removeSession(sess)

		select {
		case <-ctx.Done():
			_ = sess.Close(context.Background())
			return
		case err := <-doneChan:
			if err != nil {
				log.Error("session failed", slog.Any("err", err))
			}

		}
	}
}

type ServerConfig struct {
	Addr        string
	Path        string
	AuthHandler func(req *http.Request) error
	Debug       bool
	// AudioFormat preconfigures accepted transports for voice-server sessions.
	// Zero leaves audio unconfigured so application servers can select it dynamically.
	AudioFormat rtvbp.MediaFormat
	// KeepalivePolicy enables transport-native Ping/Pong monitoring for accepted sessions.
	// Zero disables keepalive.
	KeepalivePolicy rtvbp.KeepalivePolicy
	// Subprotocols lists supported RTVBP profiles in server preference order.
	// Nil defaults to rtvbp.v1; clients that send no offer use that profile implicitly.
	Subprotocols []string
	// AcceptedTransport optionally decorates an authenticated, upgraded semantic WebSocket before
	// the session starts. Composite bindings such as WebRTC+WebSocket use it for transport-private
	// negotiation while reusing this server's admission, control, keepalive, and close behavior.
	AcceptedTransport func(context.Context, rtvbp.Envelope, *Transport) (rtvbp.Transport, error)
}

func (c ServerConfig) Validate() error {
	if err := validateOptionalAudioFormat(c.AudioFormat); err != nil {
		return err
	}
	if err := c.KeepalivePolicy.Validate(); err != nil {
		return fmt.Errorf("invalid keepalive policy: %w", err)
	}
	return nil
}

func (c *ServerConfig) Defaults() {
	if c.Addr == "" {
		c.Addr = "127.0.0.1:8080"
	}
	if c.Path == "" {
		c.Path = "/"
	}
	c.Subprotocols = defaultSubprotocols(c.Subprotocols)
}

type Server struct {
	logger   *slog.Logger
	config   ServerConfig
	addr     *net.TCPAddr
	http     *http.Server
	listener net.Listener
	mu       sync.Mutex
	sessions map[string]*rtvbp.Session

	shuttingDown  bool
	admissions    int
	admissionIdle chan struct{}

	// afterUpgrade is a deterministic test barrier for the hijack-to-admission window.
	afterUpgrade func()
}

func (s *Server) beginAdmission() (func(), bool) {
	s.mu.Lock()
	if s.shuttingDown {
		s.mu.Unlock()
		return nil, false
	}
	if s.admissions == 0 {
		s.admissionIdle = make(chan struct{})
	}
	s.admissions++
	s.mu.Unlock()

	var once sync.Once
	return func() {
		once.Do(func() {
			s.mu.Lock()
			s.admissions--
			if s.admissions == 0 {
				close(s.admissionIdle)
			}
			s.mu.Unlock()
		})
	}, true
}

func (s *Server) addSession(sess *rtvbp.Session) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.shuttingDown {
		return false
	}
	s.sessions[sess.ID()] = sess
	s.logger.Info("session added", slog.String("session", sess.ID()))
	return true
}

func (s *Server) removeSession(sess *rtvbp.Session) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.sessions, sess.ID())
	s.logger.Info("session removed", slog.String("session", sess.ID()))
}

func (s *Server) Shutdown(ctx context.Context) (err error) {
	s.mu.Lock()
	s.shuttingDown = true
	sessions := make([]*rtvbp.Session, 0, len(s.sessions))
	for _, sess := range s.sessions {
		sessions = append(sessions, sess)
	}
	admissionIdle := s.admissionIdle
	s.mu.Unlock()

	var closeErr error
	for _, sess := range sessions {
		closeErr = errors.Join(closeErr, sess.Close(ctx))
	}

	httpErr := s.http.Shutdown(ctx)
	if admissionIdle != nil {
		select {
		case <-admissionIdle:
		case <-ctx.Done():
			err = errors.Join(closeErr, httpErr, ctx.Err())
			s.logger.Info("shutdown complete", slog.Any("err", err))
			return err
		}
	}
	err = errors.Join(closeErr, httpErr)
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
		AudioFormat:  s.config.AudioFormat,
		Subprotocols: append([]string(nil), s.config.Subprotocols...),
	}
}

func (s *Server) NewClientSession(handler rtvbp.SessionHandler) *rtvbp.Session {
	return rtvbp.NewSession(
		v1classic.Envelope{},
		Client(s.GetClientConfig()),
		rtvbp.WithHandler(handler),
	)
}

func (s *Server) Listen() error {
	if err := s.config.Validate(); err != nil {
		return err
	}
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

func NewServerWithLogger(logger *slog.Logger, config ServerConfig, handler rtvbp.SessionHandler) *Server {
	config.Defaults()

	srv := &Server{
		logger:   logger,
		config:   config,
		sessions: map[string]*rtvbp.Session{},
	}

	// handler
	mux := http.NewServeMux()
	path := config.Path
	if path == "" {
		path = "/"
	}
	mux.HandleFunc(path, serverUpgradeHandler(srv, &config, logger, handler))

	srv.http = &http.Server{
		Addr:    config.Addr,
		Handler: mux,
	}

	return srv
}

func NewServer(
	config ServerConfig,
	handler rtvbp.SessionHandler,
) *Server {
	return NewServerWithLogger(
		slog.Default().With(
			slog.String("transport", "websocket"),
			slog.String("peer", "server"),
		),
		config,
		handler,
	)
}
