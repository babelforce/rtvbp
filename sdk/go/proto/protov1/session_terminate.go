package protov1

import (
	"fmt"
)

type SessionTerminateRequest struct {
	Reason string `json:"reason"`
}

func (r *SessionTerminateRequest) Validate() error {
	if r.Reason == "" {
		return fmt.Errorf("session.terminate request reason is required")
	}
	return nil
}

func (r *SessionTerminateRequest) MethodName() string {
	return "session.terminate"
}
