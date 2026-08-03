package protov1

import (
	"fmt"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
)

const DefaultPTime = 20 * time.Millisecond

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

func DefaultMediaFormat() rtvbp.MediaFormat {
	return rtvbp.MediaFormat{
		Encoding:   AudioCodecL16_8khz_mono.Name,
		SampleRate: AudioCodecL16_8khz_mono.SampleRate,
		BitDepth:   AudioCodecL16_8khz_mono.BitDepth,
		Channels:   AudioCodecL16_8khz_mono.Channels,
		PTime:      DefaultPTime,
	}
}

func MediaFormat(codec *AudioCodec, ptime time.Duration) (rtvbp.MediaFormat, error) {
	if codec == nil {
		return rtvbp.MediaFormat{}, fmt.Errorf("audio codec is required")
	}
	if ptime == 0 {
		ptime = DefaultPTime
	}
	format := rtvbp.MediaFormat{
		Encoding:   codec.Name,
		SampleRate: codec.SampleRate,
		BitDepth:   codec.BitDepth,
		Channels:   codec.Channels,
		PTime:      ptime,
	}
	if _, err := format.FrameBytes(); err != nil {
		return rtvbp.MediaFormat{}, err
	}
	return format, nil
}

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

func audioCodec(format rtvbp.MediaFormat) AudioCodec {
	return AudioCodec{
		ID:         fmt.Sprintf("%s/%d/%d", format.Encoding, format.SampleRate, format.Channels),
		Name:       format.Encoding,
		SampleRate: format.SampleRate,
		BitDepth:   format.BitDepth,
		Channels:   format.Channels,
	}
}

type CallInfo struct {
	ID        string `json:"id"`
	SessionID string `json:"session_id"`
	From      string `json:"from"`
	To        string `json:"to"`
}

type AppInfo struct {
	ID string `json:"id"`
}

type SessionInitializeRequest struct {
	AppInfo             AppInfo        `json:"application"`
	CallInfo            CallInfo       `json:"call"`
	AudioCodecOfferings []AudioCodec   `json:"audio_codec_offerings"`
	Metadata            map[string]any `json:"metadata"`
}

func (r *SessionInitializeRequest) MethodName() string {
	return "session.initialize"
}

type SessionInitializeResponse struct {
	AudioCodec *AudioCodec `json:"audio_codec"`
}

type SessionUpdatedEvent struct {
	AudioCodec *AudioCodec `json:"audio_codec"`
}

func (e *SessionUpdatedEvent) EventName() string {
	return "session.updated"
}

func (e *SessionUpdatedEvent) String() string {
	return "SessionInitializeRequest"
}
