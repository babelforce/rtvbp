package rtvbp

import "github.com/babelforce/rtvbp-go/proto"

type sessionMessage struct {
	Version  string               `json:"version,omitempty"`
	ID       string               `json:"id,omitempty"`
	Method   string               `json:"method,omitempty"`
	Response string               `json:"response,omitempty"`
	Event    string               `json:"event,omitempty"`
	Data     any                  `json:"data,omitempty"`
	Params   any                  `json:"params,omitempty"`
	Result   any                  `json:"result,omitempty"`
	Error    *proto.ResponseError `json:"error,omitempty"`
}
