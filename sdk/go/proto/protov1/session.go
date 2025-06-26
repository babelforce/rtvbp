package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
)

type AudioConfig struct {
	Format     string `json:"format"`
	SampleRate int    `json:"sample_rate"`
	Channels   int    `json:"channels"`
}

type SessionUpdatedEvent struct {
	Audio    *AudioConfig   `json:"audio"`
	Metadata map[string]any `json:"metadata"`
}

func (e *SessionUpdatedEvent) EventName() string {
	return "session.updated"
}

type SessionTerminateRequest struct {
	Reason string `json:"reason"`
}

func (r *SessionTerminateRequest) MethodName() string {
	return "session.terminate"
}

func (r *SessionTerminateRequest) PostResponseHook(ctx context.Context, hc rtvbp.SHC) error {
	return hc.Notify(ctx, &SessionTerminatedEvent{})
}

type SessionTerminateResponse struct {
}

type SessionTerminatedEvent struct {
}

func (e *SessionTerminatedEvent) EventName() string {
	return "session.terminated"
}
