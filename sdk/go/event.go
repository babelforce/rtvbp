package rtvbp

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto"
)

type NamedEvent interface {
	EventName() string
}

type EventHandler interface {
	EventName() string
	Handle(ctx context.Context, hc HandlerCtx, evt *proto.Event) error
}

// Generic typed event handler
type typedEventHandler[T NamedEvent] struct {
	name string
	h    func(context.Context, HandlerCtx, T) error
}

func (t *typedEventHandler[T]) EventName() string {
	return t.name
}

func (t *typedEventHandler[T]) Handle(ctx context.Context, h HandlerCtx, evt *proto.Event) error {
	raw, err := json.Marshal(evt.Data)
	if err != nil {
		return fmt.Errorf("marshal data: %w", err)
	}

	var data T
	if err := json.Unmarshal(raw, &data); err != nil {
		return fmt.Errorf("unmarshal into type: %w", err)
	}

	return t.h(ctx, h, data)
}

// HandleEvent creates a new typed event handler
func HandleEvent[T NamedEvent](handler func(context.Context, HandlerCtx, T) error) EventHandler {
	var zero T
	return &typedEventHandler[T]{
		name: zero.EventName(),
		h:    handler,
	}
}
