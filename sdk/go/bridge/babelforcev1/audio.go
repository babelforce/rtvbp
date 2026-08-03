package babelforcev1

import (
	"fmt"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

const DefaultPTime = 20 * time.Millisecond

var AudioCodecL16_8kHzMono = newL16Codec(8_000)

func DefaultMediaFormat() rtvbp.MediaFormat {
	return rtvbp.MediaFormat{
		Encoding:   AudioCodecL16_8kHzMono.Name,
		SampleRate: AudioCodecL16_8kHzMono.SampleRate,
		BitDepth:   AudioCodecL16_8kHzMono.BitDepth,
		Channels:   AudioCodecL16_8kHzMono.Channels,
		PTime:      DefaultPTime,
	}
}

func MediaFormat(codec *babelforcev1.AudioCodec, ptime time.Duration) (rtvbp.MediaFormat, error) {
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

func newL16Codec(sampleRate int) babelforcev1.AudioCodec {
	if sampleRate == 0 {
		sampleRate = 8_000
	}
	return babelforcev1.AudioCodec{
		ID:         fmt.Sprintf("L16/%d/1", sampleRate),
		Name:       "L16",
		SampleRate: sampleRate,
		BitDepth:   16,
		Channels:   1,
	}
}

func audioCodec(format rtvbp.MediaFormat) babelforcev1.AudioCodec {
	return babelforcev1.AudioCodec{
		ID:         fmt.Sprintf("%s/%d/%d", format.Encoding, format.SampleRate, format.Channels),
		Name:       format.Encoding,
		SampleRate: format.SampleRate,
		BitDepth:   format.BitDepth,
		Channels:   format.Channels,
	}
}
