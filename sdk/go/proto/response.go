package proto

import (
	"encoding/json"
)

type Response struct {
	messageBase
	Version  string         `json:"version,omitempty"`
	Response string         `json:"response"`
	Result   any            `json:"result,omitempty"`
	Error    *ResponseError `json:"error,omitempty"`
}

func (r *Response) MessageType() string {
	return "response"
}

func As[R any](v any) (*R, error) {
	data, err := json.Marshal(v)
	if err != nil {
		return nil, err
	}

	var r R
	if err := json.Unmarshal(data, &r); err != nil {
		return nil, err
	}

	return &r, nil
}

func (r *Response) Ok() bool {
	return r.Error == nil
}

var _ Message = &Response{}
