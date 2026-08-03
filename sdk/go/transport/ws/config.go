package ws

import (
	"fmt"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
)

type TransportConfig struct {
	Logger      *slog.Logger
	AudioFormat rtvbp.MediaFormat
}

func (c TransportConfig) Validate() error {
	return validateOptionalAudioFormat(c.AudioFormat)
}

func validateOptionalAudioFormat(format rtvbp.MediaFormat) error {
	if format == (rtvbp.MediaFormat{}) {
		return nil
	}
	if _, err := format.FrameBytes(); err != nil {
		return fmt.Errorf("invalid audio format: %w", err)
	}
	return nil
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
