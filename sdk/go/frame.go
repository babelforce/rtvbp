package rtvbp

import (
	"encoding/json"
	"time"
)

// Kind identifies the semantic control-frame shape above an envelope codec.
type Kind uint8

const (
	KindRequest Kind = iota + 1
	KindResponse
	KindEvent
)

// ControlFrame is the envelope-independent semantic unit consumed by the session runtime.
// Only an Envelope codec converts this value to or from wire bytes.
type ControlFrame struct {
	// Kind identifies whether this is a request, response, or event.
	Kind Kind
	// ID identifies a request or event. It is empty when the envelope has no message identifier.
	ID string
	// CorrelID identifies the request answered by a response.
	CorrelID string
	// Method is the request method or event name.
	Method string
	// Payload contains request params, a response result, or event data.
	Payload json.RawMessage
	// Err is set only for an error response.
	Err *WireError
	// ReceivedAt is assigned by the transport after receipt, not by the envelope codec.
	ReceivedAt time.Time
}

// WireError is an envelope-independent response error.
type WireError struct {
	Code    int
	Message string
	Data    json.RawMessage
}

// Envelope encodes and decodes semantic control frames. Implementations are stateless and pure.
type Envelope interface {
	Name() string
	Encode(ControlFrame) ([]byte, error)
	Decode([]byte) (ControlFrame, error)
}
