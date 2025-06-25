package proto

import "fmt"

type ErrorCode int

const (
	ErrUnknown ErrorCode = iota
)

type ResponseError struct {
	Code    ErrorCode `json:"code"`
	Message string    `json:"message"`
	Data    any       `json:"any,omitempty"`
}

func (e *ResponseError) Error() string {
	return fmt.Sprintf("%d: %s", e.Code, e.Message)
}

func NewError(code ErrorCode, message string) *ResponseError {
	return &ResponseError{
		Code:    code,
		Message: message,
	}
}
