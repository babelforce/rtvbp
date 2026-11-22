package rtvbp_test

import (
	"bytes"
	"context"
	"io"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
	"go.uber.org/goleak"
)

// gatedMediaTransport hands out media channels whose ReadFrame waits for the test. A frame the
// peer already enqueued therefore stays in the transport queue, exactly where it sits when a
// terminate request is processed before the media pump has run.
type gatedMediaTransport struct {
	rtvbp.Transport
	gate chan struct{}
}

func (g gatedMediaTransport) AcceptMedia(ctx context.Context) (rtvbp.MediaChannel, error) {
	media, err := g.Transport.AcceptMedia(ctx)
	if err != nil {
		return nil, err
	}
	return gatedMediaChannel{MediaChannel: media, gate: g.gate}, nil
}

type gatedMediaChannel struct {
	rtvbp.MediaChannel
	gate <-chan struct{}
}

func (c gatedMediaChannel) ReadFrame() (rtvbp.MediaFrame, error) {
	<-c.gate
	return c.MediaChannel.ReadFrame()
}

func TestSessionDeliversQueuedInboundAudioQueuedBeforeShutdown(t *testing.T) {
	defer goleak.VerifyNone(t)

	local, peer := memory.NewPair(memory.WithMedia())
	gate := make(chan struct{})
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
	session, done := runSession(t, gatedMediaTransport{Transport: local, gate: gate}, handler)
	shc := receive(t, handlerContext)

	frameBytes, err := testAudioFormat.FrameBytes()
	if err != nil {
		t.Fatalf("FrameBytes() error = %v", err)
	}
	frame := bytes.Repeat([]byte{0x5a}, frameBytes)
	if err := media.WriteFrame(rtvbp.MediaFrame{Data: frame}); err != nil {
		t.Fatalf("WriteFrame() error = %v", err)
	}

	closed := make(chan error, 1)
	go func() { closed <- session.Close(context.Background()) }()
	waitAudioWriteRefused(t, shc)
	close(gate)

	got := make([]byte, frameBytes)
	if _, err := io.ReadFull(shc.AudioStream(), got); err != nil {
		t.Fatalf("AudioStream.Read() = %v, want the frame that was queued before the session closed", err)
	}
	if !bytes.Equal(got, frame) {
		t.Fatalf("AudioStream.Read() = %v, want %v", got, frame)
	}
	if _, err := shc.AudioStream().Read(got[:1]); err != io.EOF {
		t.Fatalf("AudioStream.Read() after the queued frame = %v, want EOF", err)
	}
	if err := receive(t, closed); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
}

// waitAudioWriteRefused returns once the session stopped accepting handler audio, which is the
// moment shutdown has begun, so the test opens the gate only after that point.
func waitAudioWriteRefused(t *testing.T, shc rtvbp.SHC) {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if _, err := shc.AudioStream().Write([]byte{0}); err != nil {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatal("session never refused audio writes after Close")
}
