package webrtcws

import (
	"context"
	"errors"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/pion/webrtc/v4"
)

func TestMediaChannelValidatesFormatAndFrameSize(t *testing.T) {
	track, err := webrtc.NewTrackLocalStaticSample(
		webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypePCMU, ClockRate: pcmuClockRate, Channels: 1},
		audioID,
		"test",
	)
	if err != nil {
		t.Fatal(err)
	}
	channel := newMediaChannel(track, rtvbp.MediaFormat{})
	format := testAudioFormat()
	if err := channel.configure(format); err != nil {
		t.Fatalf("configure: %v", err)
	}
	if err := channel.WriteFrame(rtvbp.MediaFrame{Data: make([]byte, 319)}); err == nil {
		t.Fatal("short L16 frame accepted")
	}
	if err := channel.WriteFrame(rtvbp.MediaFrame{Data: make([]byte, 320)}); err != nil {
		t.Fatalf("write complete frame: %v", err)
	}
	if err := channel.configure(rtvbp.MediaFormat{Encoding: "L16", SampleRate: 16_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}); err == nil {
		t.Fatal("unsupported sample rate accepted")
	}
}

func TestWaitConnectedHonorsCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := waitConnected(ctx, make(chan error)); !errors.Is(err, context.Canceled) {
		t.Fatalf("waitConnected error = %v, want context.Canceled", err)
	}
}

func TestPeerFailureUnblocksConnectionAndMedia(t *testing.T) {
	track, err := webrtc.NewTrackLocalStaticSample(
		webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypePCMU, ClockRate: pcmuClockRate, Channels: 1},
		audioID,
		"test",
	)
	if err != nil {
		t.Fatal(err)
	}
	transport := &Transport{
		media:     newMediaChannel(track, testAudioFormat()),
		connected: make(chan error, 1),
	}
	transport.handleConnectionState(webrtc.PeerConnectionStateFailed)
	if err := <-transport.connected; !errors.Is(err, errPeerFailed) {
		t.Fatalf("connection error = %v, want %v", err, errPeerFailed)
	}
	if _, err := transport.media.ReadFrame(); !errors.Is(err, errPeerFailed) {
		t.Fatalf("media error = %v, want %v", err, errPeerFailed)
	}
}

func testAudioFormat() rtvbp.MediaFormat {
	return rtvbp.MediaFormat{Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}
}
