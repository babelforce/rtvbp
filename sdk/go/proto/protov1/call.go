package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
)

type CallHangupEvent struct {
}

func (e *CallHangupEvent) EventName() string {
	return "call.hangup"
}

type CallHangupRequest struct {
	Reason string `json:"reason"`
}

func (r *CallHangupRequest) MethodName() string {
	return "call.hangup"
}

func (r *CallHangupRequest) OnAfterReply(ctx context.Context, hc rtvbp.SHC) error {
	return terminateAndClose(ctx, hc, "call.hangup")
}

type CallHangupResponse struct {
}
