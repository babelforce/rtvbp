package protov1

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

type CallHangupResponse struct {
}
