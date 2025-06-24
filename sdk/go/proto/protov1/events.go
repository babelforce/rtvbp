package protov1

import "github.com/babelforce/rtvbp-go"

type DummyEvent struct {
	Text string `json:"text,omitempty"`
}

func (e *DummyEvent) EventName() string {
	return "dummy"
}

var _ rtvbp.NamedEvent = &DummyEvent{}
