package proto

type Response struct {
	Version  string         `json:"version,omitempty"`
	Response string         `json:"response"`
	Result   any            `json:"result,omitempty"`
	Error    *ResponseError `json:"error,omitempty"`
}

func (r *Response) Ok() bool {
	return r.Error == nil
}
