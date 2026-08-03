package memory_test

import (
	"context"
	"errors"
	"fmt"
	"io"
	"sync"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
)

func TestControlSendCopiesData(t *testing.T) {
	left, right := memory.NewPair()
	data := []byte("hello")

	if err := left.Control().Send(context.Background(), data); err != nil {
		t.Fatalf("Send() error = %v", err)
	}
	copy(data, "xxxxx")

	received, err := right.Control().Recv(context.Background())
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	if got, want := string(received.Data), "hello"; got != want {
		t.Fatalf("Recv() data = %q, want %q", got, want)
	}
	if received.ReceivedAt.IsZero() {
		t.Fatal("Recv() returned a zero ReceivedAt")
	}
}

func TestControlCancellation(t *testing.T) {
	left, right := memory.NewPair()
	canceled, cancel := context.WithCancel(context.Background())
	cancel()

	if err := left.Control().Send(canceled, []byte("discarded")); !errors.Is(err, context.Canceled) {
		t.Fatalf("Send() error = %v, want context.Canceled", err)
	}
	if _, err := left.Control().Recv(canceled); !errors.Is(err, context.Canceled) {
		t.Fatalf("Recv() error = %v, want context.Canceled", err)
	}

	if err := left.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if _, err := right.Control().Recv(context.Background()); !errors.Is(err, io.EOF) {
		t.Fatalf("peer Recv() error = %v, want io.EOF", err)
	}
}

func TestCloseFlushesAdmittedControlFrames(t *testing.T) {
	left, right := memory.NewPair()
	if err := left.Control().Send(context.Background(), []byte("last frame")); err != nil {
		t.Fatalf("Send() error = %v", err)
	}
	if err := left.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	received, err := right.Control().Recv(context.Background())
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	if got, want := string(received.Data), "last frame"; got != want {
		t.Fatalf("Recv() data = %q, want %q", got, want)
	}
	if _, err := right.Control().Recv(context.Background()); !errors.Is(err, io.EOF) {
		t.Fatalf("Recv() after drain error = %v, want io.EOF", err)
	}
}

func TestConcurrentSendAndCloseDrainsEveryAdmittedFrame(t *testing.T) {
	left, right := memory.NewPair()
	const sends = 256

	start := make(chan struct{})
	results := make(chan sendResult, sends)
	var workers sync.WaitGroup
	workers.Add(sends)
	for i := 0; i < sends; i++ {
		go func(id int) {
			defer workers.Done()
			<-start
			payload := fmt.Sprintf("frame-%03d", id)
			results <- sendResult{payload: payload, err: left.Control().Send(context.Background(), []byte(payload))}
		}(i)
	}

	closed := make(chan error, 1)
	go func() {
		<-start
		closed <- left.Close(context.Background())
	}()
	close(start)
	workers.Wait()
	close(results)
	if err := <-closed; err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	admitted := make(map[string]struct{})
	for result := range results {
		switch {
		case result.err == nil:
			admitted[result.payload] = struct{}{}
		case errors.Is(result.err, io.ErrClosedPipe):
		default:
			t.Fatalf("Send(%q) error = %v, want nil or io.ErrClosedPipe", result.payload, result.err)
		}
	}

	observed := make(map[string]struct{})
	for {
		received, err := right.Control().Recv(context.Background())
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			t.Fatalf("Recv() error = %v", err)
		}
		observed[string(received.Data)] = struct{}{}
	}
	if len(observed) != len(admitted) {
		t.Fatalf("observed %d frames, want %d admitted frames", len(observed), len(admitted))
	}
	for payload := range admitted {
		if _, ok := observed[payload]; !ok {
			t.Errorf("admitted frame %q was not observed", payload)
		}
	}
}

func TestMediaUnsupported(t *testing.T) {
	left, _ := memory.NewPair()
	canceled, cancel := context.WithCancel(context.Background())
	cancel()

	if _, err := left.OpenMedia(canceled, "audio", rtvbp.MediaFormat{}); !errors.Is(err, rtvbp.ErrMediaUnsupported) {
		t.Fatalf("OpenMedia() error = %v, want ErrMediaUnsupported", err)
	}
	if _, err := left.AcceptMedia(canceled); !errors.Is(err, rtvbp.ErrMediaUnsupported) {
		t.Fatalf("AcceptMedia() error = %v, want ErrMediaUnsupported", err)
	}
}

func TestMediaRoundTripCopiesAndPreservesTiming(t *testing.T) {
	left, right := memory.NewPair(memory.WithMedia())
	format := rtvbp.MediaFormat{
		Encoding:   "L16",
		SampleRate: 16000,
		Channels:   1,
		PTime:      20 * time.Millisecond,
	}

	leftMedia, err := left.OpenMedia(context.Background(), "audio", format)
	if err != nil {
		t.Fatalf("OpenMedia() error = %v", err)
	}
	rightMedia, err := right.AcceptMedia(context.Background())
	if err != nil {
		t.Fatalf("AcceptMedia() error = %v", err)
	}
	if got := rightMedia.ID(); got != "audio" {
		t.Fatalf("ID() = %q, want audio", got)
	}
	if got := rightMedia.Format(); got != format {
		t.Fatalf("Format() = %#v, want %#v", got, format)
	}

	data := []byte{1, 2, 3, 4}
	want := rtvbp.MediaFrame{Data: []byte{1, 2, 3, 4}, PTS: 40 * time.Millisecond, Timed: true}
	if err := leftMedia.WriteFrame(rtvbp.MediaFrame{Data: data, PTS: want.PTS, Timed: want.Timed}); err != nil {
		t.Fatalf("WriteFrame() error = %v", err)
	}
	data[0] = 9
	received, err := rightMedia.ReadFrame()
	if err != nil {
		t.Fatalf("ReadFrame() error = %v", err)
	}
	if !equalFrame(received, want) {
		t.Fatalf("ReadFrame() = %#v, want %#v", received, want)
	}

	if err := rightMedia.WriteFrame(rtvbp.MediaFrame{Data: []byte{5}, Timed: false}); err != nil {
		t.Fatalf("reverse WriteFrame() error = %v", err)
	}
	if received, err = leftMedia.ReadFrame(); err != nil || len(received.Data) != 1 || received.Data[0] != 5 {
		t.Fatalf("reverse ReadFrame() = %#v, %v", received, err)
	}
}

func TestMediaAndAcceptUnblockOnClose(t *testing.T) {
	left, right := memory.NewPair(memory.WithMedia())
	leftMedia, err := left.OpenMedia(context.Background(), "audio", rtvbp.MediaFormat{})
	if err != nil {
		t.Fatalf("OpenMedia() error = %v", err)
	}
	if _, err := right.AcceptMedia(context.Background()); err != nil {
		t.Fatalf("AcceptMedia() error = %v", err)
	}

	readDone := make(chan error, 1)
	go func() {
		_, err := leftMedia.ReadFrame()
		readDone <- err
	}()
	if err := right.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := <-readDone; !errors.Is(err, io.EOF) {
		t.Fatalf("ReadFrame() error = %v, want io.EOF", err)
	}

	otherLeft, _ := memory.NewPair(memory.WithMedia())
	acceptDone := make(chan error, 1)
	go func() {
		_, err := otherLeft.AcceptMedia(context.Background())
		acceptDone <- err
	}()
	if err := otherLeft.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := <-acceptDone; !errors.Is(err, io.EOF) {
		t.Fatalf("AcceptMedia() error = %v, want io.EOF", err)
	}
}

func TestAcceptMediaCancellation(t *testing.T) {
	left, _ := memory.NewPair(memory.WithMedia())
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := left.AcceptMedia(ctx); !errors.Is(err, context.Canceled) {
		t.Fatalf("AcceptMedia() error = %v, want context.Canceled", err)
	}
}

type sendResult struct {
	payload string
	err     error
}

func equalFrame(got, want rtvbp.MediaFrame) bool {
	if got.PTS != want.PTS || got.Timed != want.Timed || len(got.Data) != len(want.Data) {
		return false
	}
	for i := range got.Data {
		if got.Data[i] != want.Data[i] {
			return false
		}
	}
	return true
}
