package proto

type ResponseError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func NewError(code int, message string) *ResponseError {
	return &ResponseError{
		Code:    code,
		Message: message,
	}
}
