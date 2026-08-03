package ws

import (
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
)

type TransportConfig struct {
	Logger      *slog.Logger
	AudioFormat rtvbp.MediaFormat
}

func (c *TransportConfig) Defaults() {
	if c.AudioFormat == (rtvbp.MediaFormat{}) {
		c.AudioFormat = defaultAudioFormat()
	}
}

func defaultAudioFormat() rtvbp.MediaFormat {
	return rtvbp.MediaFormat{
		Encoding:   "L16",
		SampleRate: 8_000,
		BitDepth:   16,
		Channels:   1,
		PTime:      20 * time.Millisecond,
	}
}
