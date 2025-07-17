package protov1

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"time"
)

type SessionTerminateRequest struct {
	Reason string `json:"reason"`
}

func (r *SessionTerminateRequest) MethodName() string {
	return "session.terminate"
}

func (r *SessionTerminateRequest) OnAfterReply(ctx context.Context, hc rtvbp.SHC) error {
	_ = hc.Notify(ctx, &SessionTerminatedEvent{})

	// close
	closeCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	if err := hc.Close(closeCtx); err != nil {
		return fmt.Errorf("failed to close session on session.terminate request: %w", err)
	}

	return nil

}

type SessionTerminateResponse struct {
}

type SessionTerminatedEvent struct {
}

func (e *SessionTerminatedEvent) EventName() string {
	return "session.terminated"
}
