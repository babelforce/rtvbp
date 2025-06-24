package protov1

type PingRequest struct {
	Data any `json:"data"`
}

func (r *PingRequest) MethodName() string {
	return "ping"
}

type PingResponse struct {
	Data any `json:"data"`
}
