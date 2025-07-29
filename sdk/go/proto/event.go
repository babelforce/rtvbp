package proto

type IntoEvent interface {
	IntoEvent() *Event
}

type Event struct {
	messageBase
	Version string `json:"version"`
	ID      string `json:"id,omitempty"`
	Event   string `json:"event"`
	Data    any    `json:"data,omitempty"`
}

func (e Event) MessageType() string {
	return "event"
}

func NewEvent(version string, eventName string, data any) *Event {
	return &Event{
		Version: version,
		ID:      ID(),
		Event:   eventName,
		Data:    data,
	}
}

var _ Message = &Event{}
