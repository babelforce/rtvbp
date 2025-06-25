package protov1

type AudioConfig struct {
	Format     string `json:"format"`
	SampleRate int    `json:"sample_rate"`
	Channels   int    `json:"channels"`
}

type SessionUpdatedEvent struct {
	Audio    *AudioConfig
	Metadata map[string]string
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

type SessionTerminateResponse struct {
}
