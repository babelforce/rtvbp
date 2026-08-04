package webrtcws

import (
	"encoding/binary"
	"testing"
)

func TestMuLawKnownVectors(t *testing.T) {
	tests := []struct {
		linear int16
		muLaw  byte
	}{
		{linear: 0, muLaw: 0xff},
		{linear: 1, muLaw: 0xff},
		{linear: -1, muLaw: 0x7f},
		{linear: 32124, muLaw: 0x80},
		{linear: -32124, muLaw: 0x00},
		{linear: 32767, muLaw: 0x80},
		{linear: -32768, muLaw: 0x00},
	}
	for _, test := range tests {
		if got := linearToMuLaw(test.linear); got != test.muLaw {
			t.Errorf("linearToMuLaw(%d) = %#02x, want %#02x", test.linear, got, test.muLaw)
		}
	}

	decode := map[byte]int16{0xff: 0, 0x7f: 0, 0x80: 32124, 0x00: -32124}
	for encoded, want := range decode {
		if got := muLawToLinear(encoded); got != want {
			t.Errorf("muLawToLinear(%#02x) = %d, want %d", encoded, got, want)
		}
	}
}

func TestPCMUConversionUsesLittleEndianL16(t *testing.T) {
	samples := []int16{0, 1000, -1000, 32124, -32124}
	pcm := make([]byte, len(samples)*2)
	for index, sample := range samples {
		binary.LittleEndian.PutUint16(pcm[index*2:], uint16(sample))
	}
	encoded := encodePCMU(pcm)
	if len(encoded) != len(samples) {
		t.Fatalf("encoded bytes = %d, want %d", len(encoded), len(samples))
	}
	decoded := decodePCMU(encoded)
	for index, encodedSample := range encoded {
		want := muLawToLinear(encodedSample)
		if got := int16(binary.LittleEndian.Uint16(decoded[index*2:])); got != want {
			t.Errorf("decoded sample %d = %d, want %d", index, got, want)
		}
	}
}
