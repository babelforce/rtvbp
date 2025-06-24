package proto

type IntoEvent interface {
	IntoEvent() *Event
}

type Event struct {
	Version string `json:"version"`
	ID      string `json:"id,omitempty"`
	Event   string `json:"event"`
	Data    any    `json:"data,omitempty"`
}

func NewEvent(version string, eventName string, data any) *Event {
	return &Event{
		Version: version,
		ID:      ID(),
		Event:   eventName,
		Data:    data,
	}
}
