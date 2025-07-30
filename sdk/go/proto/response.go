package proto

import (
	"encoding/json"
	"fmt"
)

type Response struct {
	messageBase
	Response string         `json:"response"`
	Result   any            `json:"result,omitempty"`
	Error    *ResponseError `json:"error,omitempty"`
}

func (r *Response) Validate() error {
	if err := r.messageBase.validateBase(); err != nil {
		return err
	}

	if r.Response == "" {
		return fmt.Errorf("response is required and needs to carry the ID of the request")
	}

	if r.Error != nil {
		if err := r.Error.Validate(); err != nil {
			return fmt.Errorf("error is invalid: %w", err)
		}
	}

	if r.Result != nil {
		if v, ok := r.Result.(validatable); ok {
			if err := v.Validate(); err != nil {
				return fmt.Errorf("result is invalid: %w", err)
			}
		}
	}

	return nil
}

func (r *Response) GetType() string {
	return "response"
}

func (r *Response) Ok() bool {
	return r.Error == nil
}

// newResponseWithVersion creates a new response with version
func newResponseWithVersion(version string, requestID string, result any, err *ResponseError) *Response {
	return &Response{
		messageBase: newBase(version),
		Response:    requestID,
		Result:      result,
		Error:       err,
	}
}

// newResponse creates a new response with default version
func newResponse(requestID string, result any, err *ResponseError) *Response {
	return newResponseWithVersion(Version, requestID, result, err)
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

var _ Message = &Response{}
