package protov1

import (
	"fmt"
)

type CallHangupEvent struct {
	Reason string `json:"reason,omitempty"`
}

func (e *CallHangupEvent) String() string {
	return "call.hangup"
}

func (e *CallHangupEvent) EventName() string {
	return "call.hangup"
}

type CallHangupRequest struct {
	Reason string `json:"reason"`
}

func (r *CallHangupRequest) Validate() error {
	if r.Reason == "" {
		return fmt.Errorf("call.hangup request reason is required")
	}
	return nil
}

func (r *CallHangupRequest) MethodName() string {
	return "call.hangup"
}
