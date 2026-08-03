package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"time"
)

type NamedEvent interface {
	EventName() string
}

type Event struct {
	ID         string
	Name       string
	Payload    json.RawMessage
	ReceivedAt time.Time
}

type EventHandler interface {
	EventName() string
	Handle(ctx context.Context, handler SHC, event Event) error
}

type typedEventHandler[T NamedEvent] struct {
	name string
	h    func(context.Context, SHC, T) error
}

func (h *typedEventHandler[T]) EventName() string { return h.name }

func (h *typedEventHandler[T]) Handle(ctx context.Context, handler SHC, event Event) error {
	var data T
	payload := event.Payload
	if len(payload) == 0 {
		payload = json.RawMessage("{}")
	}
	if err := json.Unmarshal(payload, &data); err != nil {
		return fmt.Errorf("decode %s event: %w", event.Name, err)
	}
	if validation, ok := any(data).(Validation); ok && !isNil(validation) {
		if err := validation.Validate(); err != nil {
			return fmt.Errorf("validate %s event: %w", event.Name, err)
		}
	}
	return h.h(ctx, handler, data)
}

func HandleEvent[T NamedEvent](handler func(context.Context, SHC, T) error) EventHandler {
	var zero T
	return &typedEventHandler[T]{name: zero.EventName(), h: handler}
}
