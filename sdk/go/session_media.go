package rtvbp

import (
	"context"
	"errors"
	"fmt"
	"io"
)

var (
	// ErrAudioAlreadyBound reports that the session already owns an audio media channel.
	ErrAudioAlreadyBound = errors.New("rtvbp: audio media is already bound")
	// ErrAudioFormatConflict reports an attempt to replace the session's negotiated audio format.
	ErrAudioFormatConflict = errors.New("rtvbp: audio media format conflicts with the negotiated format")
	// ErrAudioUnavailable reports that audio negotiation was attempted before a transport was ready.
	ErrAudioUnavailable = errors.New("rtvbp: audio transport is not available")
)

type audioBindState uint8

const (
	audioUnbound audioBindState = iota
	audioBinding
	audioBound
)

// OpenAudio opens and binds the session's sole duplex "audio" media channel.
// The selected fixed-width format is immutable for the lifetime of the session.
func (s *Session) OpenAudio(ctx context.Context, format MediaFormat) error {
	if _, err := format.FrameBytes(); err != nil {
		return err
	}
	transport, err := s.beginAudioBind(&format)
	if err != nil {
		return err
	}
	channel, err := transport.OpenMedia(ctx, "audio", format)
	if err != nil {
		s.abortAudioBind()
		return err
	}
	return s.finishAudioBind(channel, &format)
}

// AcceptAudio waits for and binds the session's sole duplex "audio" media channel.
// Its transport-negotiated format becomes immutable for the lifetime of the session.
func (s *Session) AcceptAudio(ctx context.Context) error {
	transport, err := s.beginAudioBind(nil)
	if err != nil {
		return err
	}
	channel, err := transport.AcceptMedia(ctx)
	if err != nil {
		s.abortAudioBind()
		return err
	}
	return s.finishAudioBind(channel, nil)
}

func (s *Session) beginAudioBind(requested *MediaFormat) (Transport, error) {
	if s.closing.Load() {
		return nil, ErrSessionClosed
	}

	s.transportMu.RLock()
	transport := s.transport
	s.transportMu.RUnlock()
	if transport == nil {
		return nil, ErrAudioUnavailable
	}

	s.mediaMu.Lock()
	defer s.mediaMu.Unlock()
	if s.closing.Load() {
		return nil, ErrSessionClosed
	}
	switch s.mediaState {
	case audioBinding:
		return nil, ErrAudioAlreadyBound
	case audioBound:
		if requested != nil && s.media != nil && s.media.Format() != *requested {
			return nil, fmt.Errorf("%w: negotiated %#v, requested %#v", ErrAudioFormatConflict, s.media.Format(), *requested)
		}
		return nil, ErrAudioAlreadyBound
	default:
		s.mediaState = audioBinding
		return transport, nil
	}
}

func (s *Session) abortAudioBind() {
	s.mediaMu.Lock()
	if s.mediaState == audioBinding {
		s.mediaState = audioUnbound
	}
	s.mediaMu.Unlock()
}

func (s *Session) finishAudioBind(channel MediaChannel, requested *MediaFormat) error {
	if channel == nil {
		s.abortAudioBind()
		return errors.New("rtvbp: transport returned a nil audio media channel")
	}
	closeWithError := func(err error) error {
		s.abortAudioBind()
		return errors.Join(err, channel.Close())
	}
	if channel.ID() != "audio" {
		return closeWithError(fmt.Errorf("rtvbp: accepted media channel %q, want %q", channel.ID(), "audio"))
	}
	format := channel.Format()
	if _, err := format.FrameBytes(); err != nil {
		return closeWithError(fmt.Errorf("rtvbp: invalid audio media format: %w", err))
	}
	if requested != nil && format != *requested {
		return closeWithError(fmt.Errorf("%w: transport returned %#v, requested %#v", ErrAudioFormatConflict, format, *requested))
	}
	if err := s.audio.setFormat(format); err != nil {
		return closeWithError(fmt.Errorf("%w: %v", ErrAudioFormatConflict, err))
	}

	s.mediaMu.Lock()
	if s.closing.Load() {
		s.mediaState = audioUnbound
		s.mediaMu.Unlock()
		return errors.Join(ErrSessionClosed, channel.Close())
	}
	s.media = channel
	s.mediaState = audioBound
	s.mediaWG.Add(2)
	s.mediaMu.Unlock()

	go s.pumpAudioInbound(channel)
	go s.pumpAudioOutbound(channel)
	return nil
}

func (s *Session) pumpAudioInbound(channel MediaChannel) {
	defer s.mediaWG.Done()
	for {
		frame, err := channel.ReadFrame()
		if err != nil {
			s.handleAudioPumpError("read", err)
			return
		}
		for remaining := frame.Data; len(remaining) > 0; {
			n, writeErr := s.audio.writeInbound(remaining)
			remaining = remaining[n:]
			if writeErr != nil {
				s.handleAudioPumpError("buffer inbound", writeErr)
				return
			}
			if n == 0 {
				s.handleAudioPumpError("buffer inbound", io.ErrNoProgress)
				return
			}
		}
	}
}

func (s *Session) pumpAudioOutbound(channel MediaChannel) {
	defer s.mediaWG.Done()
	frameBytes, err := s.audio.frameBytes()
	if err != nil {
		s.handleAudioPumpError("determine outbound frame size", err)
		return
	}
	reader := outboundAudioReader{stream: s.audio}
	for {
		data := make([]byte, frameBytes)
		_, err := io.ReadFull(reader, data)
		if err != nil {
			// io.ReadFull may have consumed a short final write. Dropping it here guarantees
			// that a session never emits a partial PTime frame during shutdown.
			s.handleAudioPumpError("buffer outbound", err)
			return
		}
		if err := channel.WriteFrame(MediaFrame{Data: data}); err != nil {
			s.handleAudioPumpError("write", err)
			return
		}
	}
}

type outboundAudioReader struct {
	stream *audioStream
}

func (r outboundAudioReader) Read(p []byte) (int, error) {
	return r.stream.readOutbound(p)
}

func (s *Session) handleAudioPumpError(operation string, err error) {
	if err == nil || s.closing.Load() {
		return
	}
	// Media closes with the transport, but the control reader may still have
	// admitted frames to drain (notably a terminal response). Let control EOF
	// own orderly session shutdown so media cannot fail pending requests first.
	if errors.Is(err, io.EOF) || errors.Is(err, io.ErrClosedPipe) {
		return
	}
	s.requestStop(fmt.Errorf("audio media %s: %w", operation, err), true)
}

func (s *Session) closeAudioMedia() error {
	s.mediaMu.Lock()
	channel := s.media
	s.mediaMu.Unlock()
	if channel == nil {
		return nil
	}
	return channel.Close()
}
