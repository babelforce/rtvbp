package webrtcws

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/google/uuid"
	"github.com/pion/webrtc/v4"
)

const (
	offerMethod       = "transport.webrtc.offer"
	maxSignalFrameLen = 1 << 20
	maxSDPLen         = 512 << 10
)

type sessionDescription struct {
	SDP string `json:"sdp"`
}

func negotiateOffer(ctx context.Context, control rtvbp.ControlChannel, envelope rtvbp.Envelope, peer *webrtc.PeerConnection) error {
	offer, err := peer.CreateOffer(nil)
	if err != nil {
		return fmt.Errorf("create WebRTC offer: %w", err)
	}
	complete := webrtc.GatheringCompletePromise(peer)
	if err := peer.SetLocalDescription(offer); err != nil {
		return fmt.Errorf("set local WebRTC offer: %w", err)
	}
	if err := awaitGathering(ctx, complete); err != nil {
		return err
	}
	local := peer.LocalDescription()
	if local == nil {
		return errors.New("webrtcws: local offer is missing")
	}
	payload, err := encodeDescription(local.SDP)
	if err != nil {
		return err
	}
	id := uuid.NewString()
	encoded, err := envelope.Encode(rtvbp.ControlFrame{
		Kind:    rtvbp.KindRequest,
		ID:      id,
		Method:  offerMethod,
		Payload: payload,
	})
	if err != nil {
		return fmt.Errorf("encode WebRTC offer: %w", err)
	}
	if len(encoded) > maxSignalFrameLen {
		return errors.New("webrtcws: encoded offer exceeds signaling limit")
	}
	if err := control.Send(ctx, encoded); err != nil {
		return fmt.Errorf("send WebRTC offer: %w", err)
	}

	received, err := control.Recv(ctx)
	if err != nil {
		return fmt.Errorf("receive WebRTC answer: %w", err)
	}
	frame, err := decodeSignal(envelope, received.Data)
	if err != nil {
		return fmt.Errorf("decode WebRTC answer: %w", err)
	}
	if frame.Kind != rtvbp.KindResponse || frame.CorrelID != id {
		return fmt.Errorf("webrtcws: unexpected answer frame kind=%d correlation=%q", frame.Kind, frame.CorrelID)
	}
	if frame.Err != nil {
		return fmt.Errorf("webrtcws: offer rejected: %d %s", frame.Err.Code, frame.Err.Message)
	}
	answer, err := decodeDescription(frame.Payload)
	if err != nil {
		return fmt.Errorf("decode WebRTC answer payload: %w", err)
	}
	if err := peer.SetRemoteDescription(webrtc.SessionDescription{Type: webrtc.SDPTypeAnswer, SDP: answer.SDP}); err != nil {
		return fmt.Errorf("set remote WebRTC answer: %w", err)
	}
	return nil
}

func negotiateAnswer(ctx context.Context, control rtvbp.ControlChannel, envelope rtvbp.Envelope, peer *webrtc.PeerConnection) error {
	received, err := control.Recv(ctx)
	if err != nil {
		return fmt.Errorf("receive WebRTC offer: %w", err)
	}
	frame, err := decodeSignal(envelope, received.Data)
	if err != nil {
		return fmt.Errorf("decode WebRTC offer: %w", err)
	}
	if frame.Kind != rtvbp.KindRequest || frame.Method != offerMethod || frame.ID == "" {
		return fmt.Errorf("webrtcws: unexpected offer frame kind=%d method=%q id=%q", frame.Kind, frame.Method, frame.ID)
	}
	offer, err := decodeDescription(frame.Payload)
	if err != nil {
		return fmt.Errorf("decode WebRTC offer payload: %w", err)
	}
	if err := peer.SetRemoteDescription(webrtc.SessionDescription{Type: webrtc.SDPTypeOffer, SDP: offer.SDP}); err != nil {
		return fmt.Errorf("set remote WebRTC offer: %w", err)
	}
	answer, err := peer.CreateAnswer(nil)
	if err != nil {
		return fmt.Errorf("create WebRTC answer: %w", err)
	}
	complete := webrtc.GatheringCompletePromise(peer)
	if err := peer.SetLocalDescription(answer); err != nil {
		return fmt.Errorf("set local WebRTC answer: %w", err)
	}
	if err := awaitGathering(ctx, complete); err != nil {
		return err
	}
	local := peer.LocalDescription()
	if local == nil {
		return errors.New("webrtcws: local answer is missing")
	}
	payload, err := encodeDescription(local.SDP)
	if err != nil {
		return err
	}
	encoded, err := envelope.Encode(rtvbp.ControlFrame{
		Kind:     rtvbp.KindResponse,
		CorrelID: frame.ID,
		Payload:  payload,
	})
	if err != nil {
		return fmt.Errorf("encode WebRTC answer: %w", err)
	}
	if len(encoded) > maxSignalFrameLen {
		return errors.New("webrtcws: encoded answer exceeds signaling limit")
	}
	if err := control.Send(ctx, encoded); err != nil {
		return fmt.Errorf("send WebRTC answer: %w", err)
	}
	return nil
}

func awaitGathering(ctx context.Context, complete <-chan struct{}) error {
	select {
	case <-complete:
		return nil
	case <-ctx.Done():
		return fmt.Errorf("gather WebRTC ICE candidates: %w", ctx.Err())
	}
}

func decodeSignal(envelope rtvbp.Envelope, encoded []byte) (rtvbp.ControlFrame, error) {
	if len(encoded) == 0 || len(encoded) > maxSignalFrameLen {
		return rtvbp.ControlFrame{}, errors.New("webrtcws: signaling frame size is invalid")
	}
	return envelope.Decode(encoded)
}

func encodeDescription(sdp string) (json.RawMessage, error) {
	if len(sdp) == 0 || len(sdp) > maxSDPLen {
		return nil, errors.New("webrtcws: SDP size is invalid")
	}
	payload, err := json.Marshal(sessionDescription{SDP: sdp})
	if err != nil {
		return nil, fmt.Errorf("encode WebRTC SDP: %w", err)
	}
	return payload, nil
}

func decodeDescription(payload json.RawMessage) (sessionDescription, error) {
	var description sessionDescription
	if err := json.Unmarshal(payload, &description); err != nil {
		return sessionDescription{}, err
	}
	if len(description.SDP) == 0 || len(description.SDP) > maxSDPLen {
		return sessionDescription{}, errors.New("webrtcws: SDP size is invalid")
	}
	return description, nil
}
