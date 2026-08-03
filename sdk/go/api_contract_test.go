package rtvbp_test

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
)

type contractEnvelope struct{}

func (contractEnvelope) Name() string { return "contract" }
func (contractEnvelope) Encode(rtvbp.ControlFrame) ([]byte, error) {
	return nil, nil
}
func (contractEnvelope) Decode([]byte) (rtvbp.ControlFrame, error) {
	return rtvbp.ControlFrame{}, nil
}

type contractControl struct{}

func (contractControl) Send(context.Context, []byte) error { return nil }
func (contractControl) Recv(context.Context) (rtvbp.Received, error) {
	return rtvbp.Received{}, nil
}

type contractMedia struct{}

func (contractMedia) ID() string                        { return "audio" }
func (contractMedia) Format() rtvbp.MediaFormat         { return rtvbp.MediaFormat{} }
func (contractMedia) WriteFrame(rtvbp.MediaFrame) error { return nil }
func (contractMedia) ReadFrame() (rtvbp.MediaFrame, error) {
	return rtvbp.MediaFrame{}, nil
}
func (contractMedia) Close() error { return nil }

type contractTransport struct{}

func (contractTransport) Control() rtvbp.ControlChannel { return contractControl{} }
func (contractTransport) AcceptMedia(context.Context) (rtvbp.MediaChannel, error) {
	return contractMedia{}, nil
}
func (contractTransport) OpenMedia(context.Context, string, rtvbp.MediaFormat) (rtvbp.MediaChannel, error) {
	return contractMedia{}, nil
}
func (contractTransport) Close(context.Context) error { return nil }

var (
	_ rtvbp.Envelope         = contractEnvelope{}
	_ rtvbp.ControlChannel   = contractControl{}
	_ rtvbp.MediaChannel     = contractMedia{}
	_ rtvbp.Transport        = contractTransport{}
	_ rtvbp.TransportFactory = func(context.Context, rtvbp.Envelope) (rtvbp.Transport, error) {
		return contractTransport{}, nil
	}
)

func compileControlFrameContract() {
	_ = rtvbp.ControlFrame{
		Kind:       rtvbp.KindResponse,
		ID:         "event-id",
		CorrelID:   "request-id",
		Method:     "session.initialize",
		Payload:    json.RawMessage(`{}`),
		Err:        &rtvbp.WireError{Code: 500, Message: "failed", Data: json.RawMessage(`null`)},
		ReceivedAt: time.Unix(1, 0),
	}
}

func TestMemoryTransportSatisfiesRuntimeCloseContract(t *testing.T) {
	left, right := memory.NewPair()
	var transport rtvbp.Transport = left

	if err := transport.Control().Send(context.Background(), []byte("final")); err != nil {
		t.Fatalf("Send() error = %v", err)
	}
	if err := transport.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	received, err := right.Control().Recv(context.Background())
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	if got, want := string(received.Data), "final"; got != want {
		t.Fatalf("Recv() data = %q, want %q", got, want)
	}
	if _, err := right.Control().Recv(context.Background()); !errors.Is(err, io.EOF) {
		t.Fatalf("Recv() after drain error = %v, want io.EOF", err)
	}
}
