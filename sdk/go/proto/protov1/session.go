package protov1

type AudioConfig struct {
	Format     string `json:"format"`
	SampleRate int    `json:"sample_rate"`
	Channels   int    `json:"channels"`
}

type SessionUpdatedEvent struct {
	Audio *AudioConfig
}

func (e *SessionUpdatedEvent) EventName() string {
	return "session.updated"
}
