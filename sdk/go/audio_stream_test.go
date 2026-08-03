package rtvbp

import (
	"errors"
	"io"
	"testing"
	"time"

	"go.uber.org/goleak"
)

func TestMediaFormatFrameBytes(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name   string
		format MediaFormat
		want   int
		ok     bool
	}{
		{name: "8k mono 20ms", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}, want: 320, ok: true},
		{name: "16k mono 20ms", format: MediaFormat{Encoding: "L16", SampleRate: 16_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}, want: 640, ok: true},
		{name: "8k stereo 10ms", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 2, PTime: 10 * time.Millisecond}, want: 320, ok: true},
		{name: "unsupported encoding", format: MediaFormat{Encoding: "opus", SampleRate: 48_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}},
		{name: "wrong bit depth", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 8, Channels: 1, PTime: 20 * time.Millisecond}},
		{name: "zero rate", format: MediaFormat{Encoding: "L16", BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}},
		{name: "zero channels", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, PTime: 20 * time.Millisecond}},
		{name: "zero ptime", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1}},
		{name: "fractional sample count", format: MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1, PTime: time.Nanosecond}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := tt.format.FrameBytes()
			if tt.ok {
				if err != nil {
					t.Fatalf("FrameBytes() error = %v", err)
				}
				if got != tt.want {
					t.Fatalf("FrameBytes() = %d, want %d", got, tt.want)
				}
				return
			}
			if err == nil {
				t.Fatalf("FrameBytes() = %d, want error", got)
			}
		})
	}
}

func TestAudioStreamNegotiatedFormatIsImmutable(t *testing.T) {
	t.Parallel()

	stream := newAudioStream(1024)
	t.Cleanup(func() { _ = stream.Close() })
	if got := stream.Format(); got != (MediaFormat{}) {
		t.Fatalf("Format() before negotiation = %#v", got)
	}

	format := MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}
	if err := stream.setFormat(format); err != nil {
		t.Fatalf("setFormat() error = %v", err)
	}
	if err := stream.setFormat(format); err != nil {
		t.Fatalf("idempotent setFormat() error = %v", err)
	}
	if got := stream.Format(); got != format {
		t.Fatalf("Format() = %#v, want %#v", got, format)
	}
	if got, err := stream.frameBytes(); err != nil || got != 320 {
		t.Fatalf("frameBytes() = %d, %v, want 320", got, err)
	}

	changed := format
	changed.PTime = 10 * time.Millisecond
	if err := stream.setFormat(changed); err == nil {
		t.Fatal("setFormat() changed an immutable negotiated format")
	}
	if got := stream.Format(); got != format {
		t.Fatalf("failed format change mutated Format() to %#v", got)
	}
}

func TestAudioStreamSeparatesInboundAndOutboundBytes(t *testing.T) {
	t.Parallel()

	stream := newAudioStream(1024)
	t.Cleanup(func() { _ = stream.Close() })

	if _, err := stream.writeInbound([]byte("from peer")); err != nil {
		t.Fatal(err)
	}
	input := make([]byte, 32)
	n, err := stream.Read(input)
	if err != nil || string(input[:n]) != "from peer" {
		t.Fatalf("Read() = %q, %v", input[:n], err)
	}

	if _, err := stream.Write([]byte("to peer")); err != nil {
		t.Fatal(err)
	}
	output := make([]byte, 32)
	n, err = stream.readOutbound(output)
	if err != nil || string(output[:n]) != "to peer" {
		t.Fatalf("readOutbound() = %q, %v", output[:n], err)
	}
}

func TestClearReadBufferDoesNotPoisonBlockedReader(t *testing.T) {
	defer goleak.VerifyNone(t)

	stream := newAudioStream(1024)
	defer stream.Close()
	read := make(chan struct {
		data string
		err  error
	}, 1)
	go func() {
		buffer := make([]byte, 16)
		n, err := stream.Read(buffer)
		read <- struct {
			data string
			err  error
		}{data: string(buffer[:n]), err: err}
	}()

	if cleared, err := stream.ClearReadBuffer(); err != nil || cleared != 0 {
		t.Fatalf("ClearReadBuffer() = %d, %v", cleared, err)
	}
	if _, err := stream.writeInbound([]byte("later")); err != nil {
		t.Fatal(err)
	}
	select {
	case got := <-read:
		if got.err != nil || got.data != "later" {
			t.Fatalf("blocked Read() = %q, %v", got.data, got.err)
		}
	case <-time.After(time.Second):
		t.Fatal("blocked Read did not resume")
	}

	if _, err := stream.writeInbound([]byte("discard")); err != nil {
		t.Fatal(err)
	}
	if cleared, err := stream.ClearReadBuffer(); err != nil || cleared != len("discard") {
		t.Fatalf("ClearReadBuffer() = %d, %v", cleared, err)
	}
}

func TestAudioStreamCloseUnblocksBothDirections(t *testing.T) {
	defer goleak.VerifyNone(t)

	stream := newAudioStream(8)
	readErrors := make(chan error, 2)
	go func() {
		_, err := stream.Read(make([]byte, 1))
		readErrors <- err
	}()
	go func() {
		_, err := stream.readOutbound(make([]byte, 1))
		readErrors <- err
	}()
	if err := stream.Close(); err != nil {
		t.Fatal(err)
	}
	for range 2 {
		select {
		case err := <-readErrors:
			if !errors.Is(err, io.EOF) {
				t.Fatalf("blocked read error = %v, want io.EOF", err)
			}
		case <-time.After(time.Second):
			t.Fatal("blocked read did not unblock")
		}
	}
}
