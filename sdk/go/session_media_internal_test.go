package rtvbp

import (
	"errors"
	"io"
	"testing"
)

func TestOrderlyAudioCloseDoesNotPreemptControlDrain(t *testing.T) {
	for _, mediaErr := range []error{io.EOF, io.ErrClosedPipe} {
		session := &Session{stop: make(chan struct{}, 1)}
		session.handleAudioPumpError("read", mediaErr)

		select {
		case <-session.stop:
			t.Fatalf("media error %v requested session shutdown before control could drain", mediaErr)
		default:
		}
		session.stopMu.Lock()
		queued := len(session.stopQueue)
		session.stopMu.Unlock()
		if queued != 0 {
			t.Fatalf("media error %v queued %d stop requests, want none", mediaErr, queued)
		}
	}
}

func TestUnexpectedAudioFailureStillFailsSession(t *testing.T) {
	want := errors.New("media failed")
	session := &Session{stop: make(chan struct{}, 1)}

	session.handleAudioPumpError("read", want)

	select {
	case <-session.stop:
	default:
		t.Fatal("unexpected media failure did not request session shutdown")
	}
	terminal := session.commitStopRequests(stopRequest{})
	if !terminal.failed || !errors.Is(terminal.cause, want) {
		t.Fatalf("terminal = %#v, want failed result containing %v", terminal, want)
	}
}
