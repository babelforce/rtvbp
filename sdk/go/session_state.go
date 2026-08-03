package rtvbp

import "log/slog"

type SessionState string

const (
	SessionStateInactive SessionState = "inactive"
	SessionStateActive   SessionState = "active"
	SessionStateClosing  SessionState = "closing"
	SessionStateClosed   SessionState = "closed"
	SessionStateFailed   SessionState = "failed"
)

func (s *Session) setState(state SessionState) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.state = state
	s.logger.Debug("Session.setState", slog.Any("state", state))
}

func (s *Session) State() SessionState {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.state
}
