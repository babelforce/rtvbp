package proto

import (
	"fmt"

	"github.com/babelforce/rtvbp-go/internal/idgen"
)

type IntoEvent interface {
	IntoEvent() *Event
}

type Event struct {
	messageBase
	ID    string `json:"id"`
	Event string `json:"event"`
	Data  any    `json:"data,omitempty"`
}

func (e *Event) Validate() error {
	if err := e.messageBase.validateBase(); err != nil {
		return err
	}

	if e.ID == "" {
		return fmt.Errorf("event ID is required")
	}

	if e.Event == "" {
		return fmt.Errorf("event name is required")
	}

	if e.Data != nil {
		if v, ok := e.Data.(validatable); ok {
			if err := v.Validate(); err != nil {
				return fmt.Errorf("event data is invalid: %w", err)
			}
		}
	}

	return nil
}

func (e *Event) GetType() string {
	return "event"
}

func newEventWithVersionAndID(version string, id string, eventName string, data any) *Event {
	return &Event{
		messageBase: newBase(version),
		ID:          id,
		Event:       eventName,
		Data:        data,
	}
}

// NewEvent creates a new event
func NewEvent(eventName string, data any) *Event {
	return newEventWithVersionAndID(Version, idgen.ID(), eventName, data)
}

var _ Message = &Event{}
