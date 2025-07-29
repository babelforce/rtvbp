package proto

import (
	"errors"
	"fmt"
)

type ErrorCode int

const (
	ErrUnknown             ErrorCode = -1
	ErrStatusBadRequest    ErrorCode = 400
	ErrInternalServerError ErrorCode = 500
)

type ResponseError struct {
	Code    ErrorCode `json:"code"`
	Message string    `json:"message"`
	Data    any       `json:"any,omitempty"`
	cause   error
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

func NewBadRequestError(cause error) *ResponseError {
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
