package rtvbp

import "log/slog"

type SessionState string

const (
	SessionStateInactive   SessionState = "inactive"
	SessionStateConnecting SessionState = "connecting"
	SessionStateActive     SessionState = "active"
	SessionStateClosing    SessionState = "closing"
	SessionStateClosed     SessionState = "closed"
	SessionStateFailed     SessionState = "failed"
)

func (s *Session) setState(state SessionState) {
	s.stateMu.Lock()
	s.state = state
	s.stateMu.Unlock()
	s.logger.Debug("session state changed", slog.Any("state", state))
}

func (s *Session) State() SessionState {
	s.stateMu.RLock()
	defer s.stateMu.RUnlock()
	return s.state
}
