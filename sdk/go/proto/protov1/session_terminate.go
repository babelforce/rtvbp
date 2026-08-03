package protov1

import (
	"context"
	"fmt"

	"github.com/babelforce/rtvbp-go"
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

func (r *SessionTerminateRequest) OnAfterReply(ctx context.Context, hc rtvbp.SHC) error {

	// close
	/*closeCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	if err := hc.Close(closeCtx); err != nil {
		return fmt.Errorf("failed to close session on session.terminate request: %w", err)
	}*/

	return nil
}
