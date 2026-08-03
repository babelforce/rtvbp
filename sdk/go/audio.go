package rtvbp

import (
	"errors"
	"fmt"
	"io"
	"runtime"
	"sync"

	"github.com/smallnest/ringbuffer"
)

// audioStream is the session-owned byte view over one duplex media channel.
// Incoming media is written to inbound; handler writes are read from outbound by the media pump.
type audioStream struct {
	inbound  *ringbuffer.RingBuffer
	outbound *ringbuffer.RingBuffer

	formatMu  sync.RWMutex
	format    MediaFormat
	formatSet bool
	closeOnce sync.Once
}

func newAudioStream(bufferSize int) *audioStream {
	if bufferSize <= 0 {
		panic("rtvbp: audio buffer size must be positive")
	}
	return &audioStream{
		inbound:  ringbuffer.New(bufferSize).SetBlocking(true),
		outbound: ringbuffer.New(bufferSize).SetBlocking(true),
	}
}

func (s *audioStream) Read(p []byte) (int, error) {
	return s.inbound.Read(p)
}

func (s *audioStream) Write(p []byte) (int, error) {
	return s.outbound.Write(p)
}

func (s *audioStream) ClearReadBuffer() (int, error) {
	return discardAvailable(s.inbound)
}

func (s *audioStream) Format() MediaFormat {
	s.formatMu.RLock()
	defer s.formatMu.RUnlock()
	return s.format
}

// setFormat records the immutable format selected by media negotiation. Repeating the same
// selection is harmless, while a different second selection is rejected.
func (s *audioStream) setFormat(format MediaFormat) error {
	if _, err := format.FrameBytes(); err != nil {
		return err
	}
	s.formatMu.Lock()
	defer s.formatMu.Unlock()
	if s.formatSet {
		if s.format == format {
			return nil
		}
		return fmt.Errorf("rtvbp: audio format is already negotiated as %#v", s.format)
	}
	s.format = format
	s.formatSet = true
	return nil
}

func (s *audioStream) frameBytes() (int, error) {
	s.formatMu.RLock()
	defer s.formatMu.RUnlock()
	if !s.formatSet {
		return 0, fmt.Errorf("rtvbp: audio format is not negotiated")
	}
	return s.format.FrameBytes()
}

func (s *audioStream) writeInbound(p []byte) (int, error) {
	return s.inbound.Write(p)
}

func (s *audioStream) readOutbound(p []byte) (int, error) {
	return s.outbound.Read(p)
}

func (s *audioStream) Close() error {
	s.closeOnce.Do(func() {
		s.inbound.CloseWithError(io.EOF)
		s.outbound.CloseWithError(io.EOF)
	})
	return nil
}

func discardAvailable(buffer *ringbuffer.RingBuffer) (int, error) {
	remaining := buffer.Length()
	if remaining == 0 {
		return 0, nil
	}
	scratch := make([]byte, min(remaining, 32*1024))
	cleared := 0
	for cleared < remaining {
		want := min(len(scratch), remaining-cleared)
		n, err := buffer.TryRead(scratch[:want])
		cleared += n
		switch {
		case err == nil:
		case errors.Is(err, ringbuffer.ErrAcquireLock):
			runtime.Gosched()
		case errors.Is(err, ringbuffer.ErrIsEmpty):
			return cleared, nil
		default:
			return cleared, err
		}
	}
	return cleared, nil
}

// AudioChannelSide is a compatibility view over the session-owned audio stream.
// New session/media code should own audioStream directly.
type AudioChannelSide struct {
	stream  *audioStream
	reverse bool
}

// Close closes the reader and writer
func (s *AudioChannelSide) Close() error {
	return s.stream.Close()
}

func (s *AudioChannelSide) Read(p []byte) (n int, err error) {
	if s.reverse {
		return s.stream.readOutbound(p)
	}
	return s.stream.Read(p)
}

func (s *AudioChannelSide) Write(p []byte) (n int, err error) {
	if s.reverse {
		return s.stream.writeInbound(p)
	}
	return s.stream.Write(p)
}

func (s *AudioChannelSide) ClearReadBuffer() (int, error) {
	if s.reverse {
		return discardAvailable(s.stream.outbound)
	}
	return s.stream.ClearReadBuffer()
}

func (s *AudioChannelSide) Format() MediaFormat {
	return s.stream.Format()
}

func NewAudioChannel(audioBufferSize int) (*AudioChannelSide, *AudioChannelSide) {
	stream := newAudioStream(audioBufferSize)
	return &AudioChannelSide{stream: stream}, &AudioChannelSide{stream: stream, reverse: true}
}
