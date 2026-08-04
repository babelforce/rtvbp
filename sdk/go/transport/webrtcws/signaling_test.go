package webrtcws

import (
	"context"
	"sync"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/pion/webrtc/v4"
)

type channelControl struct {
	send chan<- []byte
	recv <-chan []byte
}

func (c channelControl) Send(ctx context.Context, data []byte) error {
	select {
	case c.send <- append([]byte(nil), data...):
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

func (c channelControl) Recv(ctx context.Context) (rtvbp.Received, error) {
	select {
	case data := <-c.recv:
		return rtvbp.Received{Data: data, ReceivedAt: time.Now()}, nil
	case <-ctx.Done():
		return rtvbp.Received{}, ctx.Err()
	}
}

type recordingEnvelope struct {
	delegate rtvbp.Envelope
	mu       sync.Mutex
	encoded  []rtvbp.ControlFrame
}

func (e *recordingEnvelope) Name() string { return e.delegate.Name() }

func (e *recordingEnvelope) Encode(frame rtvbp.ControlFrame) ([]byte, error) {
	e.mu.Lock()
	e.encoded = append(e.encoded, frame)
	e.mu.Unlock()
	return e.delegate.Encode(frame)
}

func (e *recordingEnvelope) Decode(data []byte) (rtvbp.ControlFrame, error) {
	return e.delegate.Decode(data)
}

func TestOfferAnswerUsesReservedCorrelatedEnvelopeExchange(t *testing.T) {
	offerer, err := signalingPeer()
	if err != nil {
		t.Fatal(err)
	}
	defer offerer.Close()
	answerer, err := signalingPeer()
	if err != nil {
		t.Fatal(err)
	}
	defer answerer.Close()

	toAnswerer := make(chan []byte, 1)
	toOfferer := make(chan []byte, 1)
	offerControl := channelControl{send: toAnswerer, recv: toOfferer}
	answerControl := channelControl{send: toOfferer, recv: toAnswerer}
	envelope := &recordingEnvelope{delegate: v1classic.Envelope{}}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	answerDone := make(chan error, 1)
	go func() { answerDone <- negotiateAnswer(ctx, answerControl, envelope, answerer) }()
	if err := negotiateOffer(ctx, offerControl, envelope, offerer); err != nil {
		t.Fatalf("offer negotiation: %v", err)
	}
	if err := <-answerDone; err != nil {
		t.Fatalf("answer negotiation: %v", err)
	}

	envelope.mu.Lock()
	frames := append([]rtvbp.ControlFrame(nil), envelope.encoded...)
	envelope.mu.Unlock()
	if len(frames) != 2 {
		t.Fatalf("encoded frame count = %d, want 2", len(frames))
	}
	if frames[0].Kind != rtvbp.KindRequest || frames[0].Method != offerMethod || frames[0].ID == "" {
		t.Fatalf("offer frame = %#v", frames[0])
	}
	if frames[1].Kind != rtvbp.KindResponse || frames[1].CorrelID != frames[0].ID {
		t.Fatalf("answer frame = %#v; offer id = %q", frames[1], frames[0].ID)
	}
	if offerer.RemoteDescription() == nil || answerer.RemoteDescription() == nil {
		t.Fatal("both Pion peers must have remote descriptions")
	}
}

func TestSignalSizeLimits(t *testing.T) {
	if _, err := decodeSignal(v1classic.Envelope{}, make([]byte, maxSignalFrameLen+1)); err == nil {
		t.Fatal("oversized signaling frame accepted")
	}
	if _, err := encodeDescription(string(make([]byte, maxSDPLen+1))); err == nil {
		t.Fatal("oversized SDP accepted")
	}
}

func TestMalformedSignalPayloadAndGatheringCancellation(t *testing.T) {
	if _, err := decodeDescription([]byte(`{"sdp":""}`)); err == nil {
		t.Fatal("empty SDP accepted")
	}
	if _, err := decodeDescription([]byte(`not-json`)); err == nil {
		t.Fatal("malformed SDP payload accepted")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := awaitGathering(ctx, make(chan struct{})); err == nil {
		t.Fatal("canceled ICE gathering returned no error")
	}
}

func signalingPeer() (*webrtc.PeerConnection, error) {
	mediaEngine := &webrtc.MediaEngine{}
	if err := mediaEngine.RegisterCodec(webrtc.RTPCodecParameters{
		RTPCodecCapability: webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypePCMU, ClockRate: 8_000, Channels: 1},
		PayloadType:        0,
	}, webrtc.RTPCodecTypeAudio); err != nil {
		return nil, err
	}
	api := webrtc.NewAPI(webrtc.WithMediaEngine(mediaEngine))
	peer, err := api.NewPeerConnection(webrtc.Configuration{})
	if err != nil {
		return nil, err
	}
	if _, err := peer.AddTransceiverFromKind(webrtc.RTPCodecTypeAudio, webrtc.RTPTransceiverInit{Direction: webrtc.RTPTransceiverDirectionSendrecv}); err != nil {
		_ = peer.Close()
		return nil, err
	}
	return peer, nil
}
