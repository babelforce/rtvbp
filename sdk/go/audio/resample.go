package audio

import (
	"errors"
	"io"
	"math"
)

type ResamplingReadWriter struct {
	base     io.ReadWriter
	srcRate  float64
	dstRate  float64
	readBuf  []byte
	writeBuf []byte
}

func NewResamplingReadWriter(base io.ReadWriter, srcRate, dstRate float64) *ResamplingReadWriter {
	return &ResamplingReadWriter{
		base:    base,
		srcRate: srcRate,
		dstRate: dstRate,
	}
}

// naive linear interpolation for upsampling/downsampling PCM audio
func resample(data []byte, srcRate, dstRate float64) ([]byte, error) {
	if len(data)%2 != 0 {
		return nil, errors.New("input must be 16-bit PCM")
	}

	ratio := dstRate / srcRate
	sampleCount := len(data) / 2
	newSampleCount := int(float64(sampleCount) * ratio)

	resampled := make([]byte, newSampleCount*2)

	for i := 0; i < newSampleCount; i++ {
		srcIndex := float64(i) / ratio
		i0 := int(math.Floor(srcIndex))
		i1 := int(math.Min(float64(sampleCount-1), float64(i0+1)))
		frac := srcIndex - float64(i0)

		s0 := int16(data[i0*2]) | int16(data[i0*2+1])<<8
		s1 := int16(data[i1*2]) | int16(data[i1*2+1])<<8
		interpolated := int16(float64(s0)*(1-frac) + float64(s1)*frac)

		resampled[i*2] = byte(interpolated)
		resampled[i*2+1] = byte(interpolated >> 8)
	}

	return resampled, nil
}

// Read: reads from base, resamples to dstRate
func (r *ResamplingReadWriter) Read(p []byte) (int, error) {
	temp := make([]byte, len(p)*2) // allow more data due to downsampling
	n, err := r.base.Read(temp)
	if n == 0 {
		return 0, err
	}

	resampled, err := resample(temp[:n], r.srcRate, r.dstRate)
	if err != nil {
		return 0, err
	}

	copyLen := copy(p, resampled)
	return copyLen, err
}

// Write: resamples from dstRate back to srcRate, writes to base
func (r *ResamplingReadWriter) Write(p []byte) (int, error) {
	resampled, err := resample(p, r.dstRate, r.srcRate)
	if err != nil {
		return 0, err
	}
	return r.base.Write(resampled)
}
