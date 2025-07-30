package proto

import (
	"errors"
	"fmt"
)

type ErrorCode int

const (
	ErrUnspecified         ErrorCode = 0
	ErrUnknown             ErrorCode = -1
	ErrStatusBadRequest    ErrorCode = 400
	ErrInternalServerError ErrorCode = 500
	ErrNotImplemented      ErrorCode = 501
)

type ResponseError struct {
	Code    ErrorCode `json:"code"`
	Message string    `json:"message"`
	Data    any       `json:"any,omitempty"`
	cause   error
}

func (e *ResponseError) Validate() error {
	if e.Code == ErrUnspecified {
		return fmt.Errorf("error code is required")
	}

	if e.Message == "" {
		return fmt.Errorf("error message is required")
	}

	return nil
}

func (e *ResponseError) Unwrap() error {
	return e.cause
}

func (e *ResponseError) Error() string {
	return fmt.Sprintf("%d: %s", e.Code, e.Message)
}

func NewError(code ErrorCode, cause error) *ResponseError {
	return &ResponseError{
		cause:   cause,
		Code:    code,
		Message: cause.Error(),
	}
}

func NotImplemented(msg string) *ResponseError {
	return NewError(ErrNotImplemented, errors.New(msg))
}

func BadRequest(cause error) *ResponseError {
	return NewError(ErrStatusBadRequest, cause)
}

func ToResponseError(err error) *ResponseError {
	var re *ResponseError
	if errors.As(err, &re) {
		return re
	} else {
		return NewError(ErrInternalServerError, err)
	}
}
