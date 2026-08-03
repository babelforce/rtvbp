package proto

import (
	"fmt"

	"github.com/babelforce/rtvbp-go/internal/idgen"
)

type IntoRequest interface {
	IntoRequest() *Request
}

type Request struct {
	messageBase
	ID     string `json:"id"`
	Method string `json:"method"`
	Params any    `json:"params,omitempty"`
}

func (r *Request) Validate() error {
	if err := r.messageBase.validateBase(); err != nil {
		return err
	}

	if r.ID == "" {
		return fmt.Errorf("request ID is required")
	}

	if r.Method == "" {
		return fmt.Errorf("request method is required")
	}

	if r.Params != nil {
		if v, ok := r.Params.(validatable); ok {
			if err := v.Validate(); err != nil {
				return fmt.Errorf("request.params invalid: %w", err)
			}
		}
	}

	return nil
}

func (r *Request) GetType() string {
	return "request"
}

// Ok creates a successful response for a request
func (r *Request) Ok(result any) *Response {
	return r.newResponse(result, nil)
}

// NotOk creates a failure response for a request
func (r *Request) NotOk(err *ResponseError) *Response {
	return r.newResponse(nil, err)
}

// newResponse creates a Response from a request
func (r *Request) newResponse(res any, err *ResponseError) *Response {
	return newResponseWithVersion(r.Version, r.ID, res, err)
}

// newRequestWithIdAndVersion creates a new Request
func newRequestWithIdAndVersion(
	version string,
	id string,
	method string,
	params any,
) *Request {
	return &Request{
		messageBase: newBase(version),
		ID:          id,
		Method:      method,
		Params:      params,
	}
}

func NewRequest(method string, params any) *Request {
	return newRequestWithIdAndVersion(Version, idgen.ID(), method, params)
}

var _ Message = &Request{}
