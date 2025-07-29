package proto

import (
	"encoding/json"
	"fmt"
)

type Message interface {
	MessageType() string
	GetReceivedAt() int64
	SetReceivedAt(int64)
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
	receivedAt int64
}

func (r *messageBase) SetReceivedAt(receivedAt int64) {
	r.receivedAt = receivedAt
}

func (r *messageBase) GetReceivedAt() int64 {
	return r.receivedAt
}

func ParseMessage(raw []byte) (Message, error) {
	var msg rawMessage
	if err := json.Unmarshal(raw, &msg); err != nil {
		return nil, err
	}

	if msg.Event != "" {
		return &Event{
			Version: msg.Version,
			ID:      msg.ID,
			Event:   msg.Event,
			Data:    msg.Data,
		}, nil
	} else if msg.Method != "" {
		return &Request{
			Version: msg.Version,
			ID:      msg.ID,
			Method:  msg.Method,
			Params:  msg.Params,
		}, nil
	} else if msg.Response != "" {
		return &Response{
			Version:  msg.Version,
			Response: msg.Response,
			Result:   msg.Result,
			Error:    msg.Error,
		}, nil
	}

	return nil, fmt.Errorf("unknown message type: %s", raw)
}
