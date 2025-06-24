package rtvbp

import (
	"context"
	"log/slog"
	"sync"
	"time"
)

type Server struct {
	logger       *slog.Logger
	acceptor     Acceptor
	shutdownOnce sync.Once
	shutdown     chan struct{}
	done         chan struct{}
	handler      SessionHandler
	sessions     map[string]*Session
	mu           sync.Mutex
}

func (s *Server) Stats() map[string]interface{} {
	s.mu.Lock()
	defer s.mu.Unlock()
	return map[string]interface{}{
		"sessions": len(s.sessions),
	}
}

func (s *Server) Shutdown() error {
	s.logger.Info("shutting down")

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	close(s.shutdown)

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-s.done:
		return nil
	}
}

func (s *Server) runSession(ctx context.Context, sess *Session) {
	func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		s.sessions[sess.id] = sess
	}()

	defer func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		delete(s.sessions, sess.id)
	}()

	sess.Run(ctx)
}

func (s *Server) tearDown() {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// TODO: close async
	func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		for _, sess := range s.sessions {
			s.logger.Debug("closing session", slog.String("session_id", sess.id))
			if err := sess.CloseContext(ctx); err != nil {
				s.logger.Error("failed to close session", slog.Any("err", err))
			}
			delete(s.sessions, sess.id)
		}
	}()

	if err := s.acceptor.Shutdown(ctx); err != nil {
		s.logger.Error("failed to shutdown acceptor", slog.Any("err", err))
	}

	s.logger.Info("shut down")

	close(s.done)
}

func (s *Server) Run(ctx context.Context) error {
	if err := s.acceptor.Run(ctx); err != nil {
		return err
	}

	defer s.tearDown()

	accept := s.acceptor.Channel()
	for {
		select {
		case <-ctx.Done():
			return nil
		case <-s.shutdown:
			return nil
		case t := <-accept:
			go s.runSession(ctx, NewSession(
				t,
				SessionConfig{},
				s.handler,
			))

		}
	}
}

func NewServer(acceptor Acceptor, handler SessionHandler) *Server {
	return &Server{
		logger:   slog.Default().With(slog.String("component", "server")),
		acceptor: acceptor,
		handler:  handler,
		shutdown: make(chan struct{}),
		done:     make(chan struct{}),
		sessions: make(map[string]*Session),
	}
}
