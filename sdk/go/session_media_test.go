package rtvbp_test

import (
	"bytes"
	"context"
	"errors"
	"io"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
	"go.uber.org/goleak"
)

var testAudioFormat = rtvbp.MediaFormat{
	Encoding:   "L16",
	SampleRate: 8_000,
	BitDepth:   16,
	Channels:   1,
	PTime:      20 * time.Millisecond,
}

func TestSessionOpenAudioWritesExactPTimeFramesAndDropsPartialOnClose(t *testing.T) {
	defer goleak.VerifyNone(t)

	local, peer := memory.NewPair(memory.WithMedia())
	handlerContext := make(chan rtvbp.SHC, 1)
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, shc rtvbp.SHC) error {
		if err := shc.OpenAudio(ctx, testAudioFormat); err != nil {
			return err
		}
		handlerContext <- shc
		return nil
	}})
	session, done := runSession(t, local, handler)
	shc := receive(t, handlerContext)
	media, err := peer.AcceptMedia(testContext(t))
	if err != nil {
		t.Fatalf("AcceptMedia() error = %v", err)
	}

	frameBytes, err := testAudioFormat.FrameBytes()
	if err != nil {
		t.Fatalf("FrameBytes() error = %v", err)
	}
	first := bytes.Repeat([]byte{0x11}, frameBytes)
	second := bytes.Repeat([]byte{0x22}, frameBytes)
	partial := bytes.Repeat([]byte{0x33}, frameBytes/2)
	payload := append(append(append([]byte{}, first...), second...), partial...)
	if n, err := shc.AudioStream().Write(payload); err != nil || n != len(payload) {
		t.Fatalf("AudioStream.Write() = (%d, %v), want (%d, nil)", n, err, len(payload))
	}

	assertMediaFrame(t, media, first)
	assertMediaFrame(t, media, second)
	closeSession(t, session, done)
	if _, err := readMediaFrame(t, media); !errors.Is(err, io.EOF) {
		t.Fatalf("ReadFrame() after close = %v, want EOF without a partial frame", err)
	}
}

func TestSessionAcceptAudioConcatenatesInboundFrameBytes(t *testing.T) {
	defer goleak.VerifyNone(t)

	local, peer := memory.NewPair(memory.WithMedia())
	media, err := peer.OpenMedia(testContext(t), "audio", testAudioFormat)
	if err != nil {
		t.Fatalf("OpenMedia() error = %v", err)
	}
	handlerContext := make(chan rtvbp.SHC, 1)
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, shc rtvbp.SHC) error {
		if err := shc.AcceptAudio(ctx); err != nil {
			return err
		}
		handlerContext <- shc
		return nil
	}})
	session, done := runSession(t, local, handler)
	shc := receive(t, handlerContext)

	first := []byte{1, 2, 3, 4, 5}
	second := []byte{6, 7, 8, 9, 10, 11, 12}
	if err := media.WriteFrame(rtvbp.MediaFrame{Data: first}); err != nil {
		t.Fatalf("WriteFrame(first) error = %v", err)
	}
	if err := media.WriteFrame(rtvbp.MediaFrame{Data: second}); err != nil {
		t.Fatalf("WriteFrame(second) error = %v", err)
	}
	want := append(append([]byte{}, first...), second...)
	got := make([]byte, len(want))
	if _, err := io.ReadFull(shc.AudioStream(), got); err != nil {
		t.Fatalf("AudioStream.Read() error = %v", err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("AudioStream.Read() = %v, want %v", got, want)
	}
	if gotFormat := shc.AudioStream().Format(); gotFormat != testAudioFormat {
		t.Fatalf("AudioStream.Format() = %#v, want %#v", gotFormat, testAudioFormat)
	}
	closeSession(t, session, done)
}

func TestSessionAudioRejectsDuplicateAndConflictingBind(t *testing.T) {
	defer goleak.VerifyNone(t)

	local, peer := memory.NewPair(memory.WithMedia())
	session, done := runSession(t, local, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	if err := session.OpenAudio(testContext(t), testAudioFormat); err != nil {
		t.Fatalf("OpenAudio() error = %v", err)
	}
	if _, err := peer.AcceptMedia(testContext(t)); err != nil {
		t.Fatalf("AcceptMedia() error = %v", err)
	}
	if err := session.OpenAudio(testContext(t), testAudioFormat); !errors.Is(err, rtvbp.ErrAudioAlreadyBound) {
		t.Fatalf("duplicate OpenAudio() error = %v, want ErrAudioAlreadyBound", err)
	}
	conflicting := testAudioFormat
	conflicting.SampleRate = 16_000
	if err := session.OpenAudio(testContext(t), conflicting); !errors.Is(err, rtvbp.ErrAudioFormatConflict) {
		t.Fatalf("conflicting OpenAudio() error = %v, want ErrAudioFormatConflict", err)
	}
	if err := session.AcceptAudio(testContext(t)); !errors.Is(err, rtvbp.ErrAudioAlreadyBound) {
		t.Fatalf("duplicate AcceptAudio() error = %v, want ErrAudioAlreadyBound", err)
	}
	closeSession(t, session, done)
}

func TestSessionCloseWithoutBoundMediaAndUnsupportedOpen(t *testing.T) {
	defer goleak.VerifyNone(t)

	local, _ := memory.NewPair()
	session, done := runSession(t, local, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	if err := session.OpenAudio(testContext(t), testAudioFormat); !errors.Is(err, rtvbp.ErrMediaUnsupported) {
		t.Fatalf("OpenAudio() error = %v, want ErrMediaUnsupported", err)
	}
	closeSession(t, session, done)

	withMedia, _ := memory.NewPair(memory.WithMedia())
	unbound, unboundDone := runSession(t, withMedia, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	closeSession(t, unbound, unboundDone)
}

func assertMediaFrame(t *testing.T, media rtvbp.MediaChannel, want []byte) {
	t.Helper()
	frame, err := readMediaFrame(t, media)
	if err != nil {
		t.Fatalf("ReadFrame() error = %v", err)
	}
	if !bytes.Equal(frame.Data, want) {
		t.Fatalf("ReadFrame().Data length/content mismatch: got %d bytes, want %d", len(frame.Data), len(want))
	}
	if frame.Timed || frame.PTS != 0 {
		t.Fatalf("ReadFrame() timing = (%v, %v), want untimed", frame.Timed, frame.PTS)
	}
}

type mediaReadResult struct {
	frame rtvbp.MediaFrame
	err   error
}

func readMediaFrame(t *testing.T, media rtvbp.MediaChannel) (rtvbp.MediaFrame, error) {
	t.Helper()
	result := make(chan mediaReadResult, 1)
	go func() {
		frame, err := media.ReadFrame()
		result <- mediaReadResult{frame: frame, err: err}
	}()
	select {
	case received := <-result:
		return received.frame, received.err
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for media frame")
		return rtvbp.MediaFrame{}, nil
	}
}
