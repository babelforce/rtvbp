package proto

type Response struct {
	Version  string `json:"version,omitempty"`
	Response string `json:"response"`
	Result   any    `json:"result,omitempty"`
	Error    any    `json:"error,omitempty"`
}
