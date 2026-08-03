package protov1

import "fmt"

// DTMFEvent dispatched by telephony when the user releases a DTMF key.
type DTMFEvent struct {
	// Seq is the sequence number of the DTMF event. Seq is incremented for each DTMF event within the same session.
	Seq int `json:"seq"`

	// PressedAt is the time the DTMF key was pressed in milliseconds since the epoch.
	PressedAt int64 `json:"pressed_at"`

	// ReleasedAt is the time the DTMF key was released in milliseconds since the epoch.
	ReleasedAt int64 `json:"released_at"`

	// Digit is the DTMF digit that was pressed.
	// Example: "1", "2", etc or "*" for the "pound" key.
	Digit string `json:"digit"`
}

func (e *DTMFEvent) EventName() string {
	return "dtmf"
}

func (e *DTMFEvent) Validate() error {
	if e.Digit == "" {
		return fmt.Errorf("digit is required")
	}
	if e.Seq < 0 {
		return fmt.Errorf("seq must be positive")
	}
	if e.PressedAt < 0 {
		return fmt.Errorf("pressed_at must be positive")
	}
	if e.ReleasedAt < 0 {
		return fmt.Errorf("released_at must be positive")
	}
	if e.ReleasedAt < e.PressedAt {
		return fmt.Errorf("released_at must be greater than pressed_at")
	}
	return nil
}

func (e *DTMFEvent) String() string {
	return fmt.Sprintf("DTMFEvent{seq=%d, digit=%s, pressed_at=%d, released_at=%d, duration=%dms}", e.Seq, e.Digit, e.PressedAt, e.ReleasedAt, e.ReleasedAt-e.PressedAt)
}
