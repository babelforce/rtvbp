package protov1

import "fmt"

type AudioCodec struct {
	ID         string `json:"id"`
	Name       string `json:"name"`
	SampleRate int    `json:"sample_rate"`
	BitDepth   int    `json:"bit_depth"`
	Channels   int    `json:"channels"`
}

// AudioCodecL16_8khz_mono
// https://datatracker.ietf.org/doc/html/rfc2586
var AudioCodecL16_8khz_mono = newL16Codec(8_000)

func newL16Codec(sr int) AudioCodec {
	if sr == 0 {
		sr = 8000
	}
	return AudioCodec{
		ID:         fmt.Sprintf("L16/%d/1", sr),
		Name:       "L16",
		SampleRate: sr,
		BitDepth:   16,
		Channels:   1,
	}
}

type SessionInitializeRequest struct {
	Metadata            map[string]any `json:"metadata"`
	AudioCodecOfferings []AudioCodec   `json:"audio_codec_offerings"`
}

func (r *SessionInitializeRequest) MethodName() string {
	return "session.initialize"
}

type SessionInitializeResponse struct {
	AudioCodec *AudioCodec `json:"audio_codec"`
}

type SessionUpdatedEvent struct {
	AudioCodec *AudioCodec    `json:"audio_codec"`
	Metadata   map[string]any `json:"metadata,omitempty"`
}

func (e *SessionUpdatedEvent) EventName() string {
	return "session.updated"
}
