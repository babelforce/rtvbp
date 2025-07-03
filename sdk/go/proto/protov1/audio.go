package protov1

// AudioBufferClearRequest instructs the peer to clear its audio buffer
// When a peer receives audio data faster than realtime it will be buffered
// on that peer (it cannot be played out faster than realtime)
// To clear that buffer (interruption) use this request
type AudioBufferClearRequest struct{}

func (r *AudioBufferClearRequest) MethodName() string {
	return "audio.buffer.clear"
}

// AudioBufferClearResponse is the response to AudioBufferClearRequest
type AudioBufferClearResponse struct {
	Len int `json:"len"` // Len holds the number of bytes that were removed from the buffer
}

type AudioSpeechStartedEvent struct {
	// Origin describes where the speech started
	// Can be "sender" or "receiver"
	Origin string `json:"origin"`
}

func (e *AudioSpeechStartedEvent) EventName() string {
	return "audio.speech.started"
}
