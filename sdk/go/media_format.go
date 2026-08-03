package rtvbp

import (
	"fmt"
	"math"
	"time"
)

// FrameBytes returns the number of fixed-width PCM bytes in one packetization interval.
// The byte-oriented audio stream currently supports the frozen catalog's L16 encoding only.
func (f MediaFormat) FrameBytes() (int, error) {
	if f.Encoding != "L16" {
		return 0, fmt.Errorf("rtvbp: unsupported byte-audio encoding %q", f.Encoding)
	}
	if f.SampleRate <= 0 {
		return 0, fmt.Errorf("rtvbp: media sample rate must be positive")
	}
	if f.BitDepth != 16 {
		return 0, fmt.Errorf("rtvbp: L16 media bit depth must be 16, got %d", f.BitDepth)
	}
	if f.Channels <= 0 {
		return 0, fmt.Errorf("rtvbp: media channel count must be positive")
	}
	if f.PTime <= 0 {
		return 0, fmt.Errorf("rtvbp: media packetization time must be positive")
	}

	ptimeNanos := f.PTime.Nanoseconds()
	if int64(f.SampleRate) > math.MaxInt64/ptimeNanos {
		return 0, fmt.Errorf("rtvbp: media frame sample count overflows")
	}
	sampleNanos := int64(f.SampleRate) * ptimeNanos
	if sampleNanos%int64(time.Second) != 0 {
		return 0, fmt.Errorf("rtvbp: media packetization time does not contain a whole number of samples")
	}
	samples := sampleNanos / int64(time.Second)
	bytesPerSample := int64(f.BitDepth / 8)
	if samples > math.MaxInt64/int64(f.Channels) {
		return 0, fmt.Errorf("rtvbp: media frame channel count overflows")
	}
	frameSamples := samples * int64(f.Channels)
	if frameSamples > math.MaxInt64/bytesPerSample {
		return 0, fmt.Errorf("rtvbp: media frame byte count overflows")
	}
	frameBytes := frameSamples * bytesPerSample
	maxInt := int64(^uint(0) >> 1)
	if frameBytes <= 0 || frameBytes > maxInt {
		return 0, fmt.Errorf("rtvbp: media frame byte count is out of range")
	}
	return int(frameBytes), nil
}
