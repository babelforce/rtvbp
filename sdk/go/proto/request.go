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

func (r *Request) Ok(result any) *Response {
	return &Response{
		Version:  r.Version,
		Response: r.ID,
		Result:   result,
	}
}

func (r *Request) NotOk(err *ResponseError) *Response {
	return &Response{
		Version:  r.Version,
		Response: r.ID,
		Error:    err,
	}
}

func (r *Request) Respond(res any, err *ResponseError) *Response {
	return &Response{
		Version:  r.Version,
		Response: r.ID,
		Result:   res,
		Error:    err,
	}
}

func NewRequest(version string, method string, params any) *Request {
	return &Request{
		Version: version,
		ID:      ID(),
		Method:  method,
		Params:  params,
	}
}
