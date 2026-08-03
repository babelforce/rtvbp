package rtvbp

import (
	"testing"
	"time"
)

func TestObservableAudioPreservesFormatAndCallbacks(t *testing.T) {
	t.Parallel()

	stream := newAudioStream(1024)
	t.Cleanup(func() { _ = stream.Close() })
	format := MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}
	if err := stream.setFormat(format); err != nil {
		t.Fatal(err)
	}

	var reads, writes, cleared int
	observed := &ObservableAudio{
		ha: stream,
		o: AudioStreamObserver{
			OnRead:          func(n int) { reads += n },
			OnWrite:         func(n int) { writes += n },
			OnBufferCleared: func(n int) { cleared += n },
		},
	}
	if got := observed.Format(); got != format {
		t.Fatalf("Format() = %#v, want %#v", got, format)
	}

	if _, err := stream.writeInbound([]byte("in")); err != nil {
		t.Fatal(err)
	}
	buffer := make([]byte, 8)
	if _, err := observed.Read(buffer); err != nil {
		t.Fatal(err)
	}
	if _, err := observed.Write([]byte("out")); err != nil {
		t.Fatal(err)
	}
	if _, err := stream.writeInbound([]byte("clear")); err != nil {
		t.Fatal(err)
	}
	if _, err := observed.ClearReadBuffer(); err != nil {
		t.Fatal(err)
	}
	if reads != 2 || writes != 3 || cleared != 5 {
		t.Fatalf("callbacks read/write/clear = %d/%d/%d", reads, writes, cleared)
	}
}
