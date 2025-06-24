package proto

type IntoRequest interface {
	IntoRequest() *Request
}

type Request struct {
	Version string `json:"version"`
	ID      string `json:"id"`
	Method  string `json:"method"`
	Params  any    `json:"params,omitempty"`
}

func NewRequest(version string, method string, params any) *Request {
	return &Request{
		Version: version,
		ID:      ID(),
		Method:  method,
		Params:  params,
	}
}
