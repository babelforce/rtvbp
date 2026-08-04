// Package webrtcws implements the webrtcws.v1 binding: RTVBP control remains on a semantic
// WebSocket transport while one duplex audio stream is carried by Pion WebRTC.
package webrtcws

import (
	"context"
	"errors"
	"fmt"
	"io"
	"sync"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/pion/interceptor"
	"github.com/pion/webrtc/v4"
)

// Subprotocol selects the WebRTC-audio plus WebSocket-control binding.
const Subprotocol = "rtvbp.webrtc.v1"

// Config controls the Pion peer and the format exposed at the Go SDK media boundary.
type Config struct {
	PeerConnection webrtc.Configuration
	AudioFormat    rtvbp.MediaFormat
}

// Transport combines a semantic WebSocket control channel with Pion WebRTC audio.
type Transport struct {
	base      *ws.Transport
	peer      *webrtc.PeerConnection
	media     *mediaChannel
	connected chan error

	claimMu sync.Mutex
	claimed bool

	closeOnce sync.Once
	closeErr  error
	stateOnce sync.Once
}

func newTransport(base *ws.Transport, config Config) (*Transport, error) {
	if base == nil {
		return nil, errors.New("webrtcws: base WebSocket transport is required")
	}
	if config.AudioFormat != (rtvbp.MediaFormat{}) {
		if err := validateFormat(config.AudioFormat); err != nil {
			return nil, err
		}
	}

	mediaEngine := &webrtc.MediaEngine{}
	if err := mediaEngine.RegisterCodec(webrtc.RTPCodecParameters{
		RTPCodecCapability: webrtc.RTPCodecCapability{
			MimeType:  webrtc.MimeTypePCMU,
			ClockRate: pcmuClockRate,
			Channels:  1,
		},
		PayloadType: 0,
	}, webrtc.RTPCodecTypeAudio); err != nil {
		return nil, fmt.Errorf("register WebRTC PCMU codec: %w", err)
	}
	registry := &interceptor.Registry{}
	if err := webrtc.RegisterDefaultInterceptors(mediaEngine, registry); err != nil {
		return nil, fmt.Errorf("register WebRTC interceptors: %w", err)
	}
	api := webrtc.NewAPI(webrtc.WithMediaEngine(mediaEngine), webrtc.WithInterceptorRegistry(registry))
	peer, err := api.NewPeerConnection(config.PeerConnection)
	if err != nil {
		return nil, fmt.Errorf("create WebRTC peer connection: %w", err)
	}
	track, err := webrtc.NewTrackLocalStaticSample(
		webrtc.RTPCodecCapability{MimeType: webrtc.MimeTypePCMU, ClockRate: pcmuClockRate, Channels: 1},
		audioID,
		"rtvbp",
	)
	if err != nil {
		_ = peer.Close()
		return nil, fmt.Errorf("create WebRTC audio track: %w", err)
	}
	transceiver, err := peer.AddTransceiverFromTrack(track, webrtc.RTPTransceiverInit{Direction: webrtc.RTPTransceiverDirectionSendrecv})
	if err != nil {
		_ = peer.Close()
		return nil, fmt.Errorf("add WebRTC audio transceiver: %w", err)
	}

	transport := &Transport{
		base:      base,
		peer:      peer,
		media:     newMediaChannel(track, config.AudioFormat),
		connected: make(chan error, 1),
	}
	go drainRTCP(transceiver.Sender())
	peer.OnTrack(func(remote *webrtc.TrackRemote, _ *webrtc.RTPReceiver) {
		if remote.Kind() != webrtc.RTPCodecTypeAudio || remote.Codec().MimeType != webrtc.MimeTypePCMU || remote.Codec().ClockRate != pcmuClockRate {
			transport.media.fail(fmt.Errorf("webrtcws: unexpected remote codec %s/%d", remote.Codec().MimeType, remote.Codec().ClockRate))
			return
		}
		go transport.media.receive(remote)
	})
	peer.OnConnectionStateChange(transport.handleConnectionState)
	return transport, nil
}

func (t *Transport) handleConnectionState(state webrtc.PeerConnectionState) {
	switch state {
	case webrtc.PeerConnectionStateConnected:
		t.stateOnce.Do(func() { t.connected <- nil })
	case webrtc.PeerConnectionStateFailed:
		t.stateOnce.Do(func() { t.connected <- errPeerFailed })
		t.media.fail(errPeerFailed)
	case webrtc.PeerConnectionStateClosed:
		t.stateOnce.Do(func() { t.connected <- io.EOF })
	}
}

func drainRTCP(sender *webrtc.RTPSender) {
	for {
		if _, _, err := sender.ReadRTCP(); err != nil {
			return
		}
	}
}

// Control returns the existing WebSocket text control channel.
func (t *Transport) Control() rtvbp.ControlChannel { return t.base.Control() }

// Subprotocol returns the effective WebSocket binding profile.
func (t *Transport) Subprotocol() string { return t.base.Subprotocol() }

// WireSubprotocol returns the selected WebSocket subprotocol.
func (t *Transport) WireSubprotocol() string { return t.base.WireSubprotocol() }

// MonitorKeepalive delegates liveness monitoring to native WebSocket Ping/Pong.
func (t *Transport) MonitorKeepalive(ctx context.Context, policy rtvbp.KeepalivePolicy) error {
	return t.base.MonitorKeepalive(ctx, policy)
}

// OpenMedia claims and configures the sole WebRTC audio channel.
func (t *Transport) OpenMedia(ctx context.Context, id string, format rtvbp.MediaFormat) (rtvbp.MediaChannel, error) {
	if id != audioID {
		return nil, rtvbp.ErrMediaUnsupported
	}
	if err := t.claim(); err != nil {
		return nil, err
	}
	if err := t.media.configure(format); err != nil {
		t.unclaim()
		return nil, err
	}
	if err := waitConnected(ctx, t.connected); err != nil {
		t.unclaim()
		return nil, err
	}
	return t.media, nil
}

// AcceptMedia claims the preconfigured sole WebRTC audio channel.
func (t *Transport) AcceptMedia(ctx context.Context) (rtvbp.MediaChannel, error) {
	if err := t.claim(); err != nil {
		return nil, err
	}
	if err := validateFormat(t.media.Format()); err != nil {
		t.unclaim()
		return nil, fmt.Errorf("webrtcws: accepted audio format is not configured: %w", err)
	}
	if err := waitConnected(ctx, t.connected); err != nil {
		t.unclaim()
		return nil, err
	}
	return t.media, nil
}

func (t *Transport) claim() error {
	t.claimMu.Lock()
	defer t.claimMu.Unlock()
	if t.claimed {
		return errMediaClaimed
	}
	t.claimed = true
	return nil
}

func (t *Transport) unclaim() {
	t.claimMu.Lock()
	t.claimed = false
	t.claimMu.Unlock()
}

// Close closes Pion media and then flushes and closes WebSocket control.
func (t *Transport) Close(ctx context.Context) error {
	t.closeOnce.Do(func() {
		_ = t.media.Close()
		peerErr := t.peer.Close()
		baseErr := t.base.Close(ctx)
		t.closeErr = errors.Join(peerErr, baseErr)
	})
	return t.closeErr
}
