package webrtcws

import "encoding/binary"

const (
	muLawBias = 0x84
	muLawClip = 32635
)

func encodePCMU(pcm []byte) []byte {
	encoded := make([]byte, len(pcm)/2)
	for index := range encoded {
		encoded[index] = linearToMuLaw(int16(binary.LittleEndian.Uint16(pcm[index*2:])))
	}
	return encoded
}

func decodePCMU(encoded []byte) []byte {
	pcm := make([]byte, len(encoded)*2)
	for index, value := range encoded {
		binary.LittleEndian.PutUint16(pcm[index*2:], uint16(muLawToLinear(value)))
	}
	return pcm
}

func linearToMuLaw(sample int16) byte {
	value := int(sample)
	sign := 0
	if value < 0 {
		sign = 0x80
		value = -value
	}
	if value > muLawClip {
		value = muLawClip
	}
	value += muLawBias

	exponent := 7
	for mask := 0x4000; exponent > 0 && value&mask == 0; mask >>= 1 {
		exponent--
	}
	mantissa := (value >> (exponent + 3)) & 0x0f
	return ^byte(sign | exponent<<4 | mantissa)
}

func muLawToLinear(encoded byte) int16 {
	value := ^encoded
	magnitude := (int(value&0x0f) << 3) + muLawBias
	magnitude <<= (value & 0x70) >> 4
	if value&0x80 != 0 {
		return int16(muLawBias - magnitude)
	}
	return int16(magnitude - muLawBias)
}
