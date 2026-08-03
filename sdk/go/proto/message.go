package proto

import (
	"encoding/json"
	"fmt"
)

const (
	Version = "1"
)

type validatable interface {
	Validate() error // Validate validates the message
}

type Message interface {
	validatable
	GetType() string      // GetType gets the message type which is either `request`, `response` or `event`
	GetReceivedAt() int64 // GetReceivedAt gets the timestamp of when the message was received
	SetReceivedAt(int64)  // SetReceivedAt sets the receival timestamp
}

type rawMessage struct {
	Version  string         `json:"version,omitempty"`
	ID       string         `json:"id,omitempty"`
	Method   string         `json:"method,omitempty"`
	Response string         `json:"response,omitempty"`
	Event    string         `json:"event,omitempty"`
	Data     any            `json:"data,omitempty"`
	Params   any            `json:"params,omitempty"`
	Result   any            `json:"result,omitempty"`
	Error    *ResponseError `json:"error,omitempty"`
}

type messageBase struct {
	Version    string `json:"version"`
	receivedAt int64
}

func (r *messageBase) validateBase() error {
	if r.Version == "" {
		return fmt.Errorf("version is required")
	}

	if r.Version != Version {
		return fmt.Errorf("unsupported version: %s - only %s is allowed", r.Version, Version)
	}

	return nil
}

func (r *messageBase) SetReceivedAt(receivedAt int64) {
	r.receivedAt = receivedAt
}

func (r *messageBase) GetReceivedAt() int64 {
	return r.receivedAt
}

func newBase(v string) messageBase {
	return messageBase{
		Version: v,
	}
}

// ParseValidMessage will parse the message and also validate its content
func ParseValidMessage(raw []byte) (m Message, err error) {
	// parse
	m, err = parseMessage(raw)
	if err != nil {
		return nil, err
	}

	// validate
	err = m.Validate()
	if err != nil {
		return nil, fmt.Errorf("failed to validate `%s` message: %s: %w", m.GetType(), raw, err)
	}

	return m, nil
}

type ParseError struct {
	Raw   []byte
	Cause error
}

func (e *ParseError) Error() string {
	return fmt.Sprintf("failed to parse message: %s", e.Cause)
}

func (e *ParseError) Unwrap() error {
	return e.Cause
}

func parseMessage(raw []byte) (Message, error) {
	var msg rawMessage
	if err := json.Unmarshal(raw, &msg); err != nil {
		return nil, err
	}

	if msg.Event != "" {
		return newEventWithVersionAndID(msg.Version, msg.ID, msg.Event, msg.Data), nil
	} else if msg.Method != "" {
		return newRequestWithIdAndVersion(msg.Version, msg.ID, msg.Method, msg.Params), nil
	} else if msg.Response != "" {
		return newResponseWithVersion(msg.Version, msg.Response, msg.Result, msg.Error), nil
	}

	return nil, &ParseError{
		Raw:   raw,
		Cause: fmt.Errorf("message is invalid: %s", string(raw)),
	}
}
