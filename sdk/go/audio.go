package rtvbp

import (
	"io"

	"github.com/smallnest/ringbuffer"
)

type AudioChannelSide struct {
	reader *ringbuffer.RingBuffer
	writer *ringbuffer.RingBuffer
}

// Close closes the reader and writer
func (s *AudioChannelSide) Close() error {
	s.writer.CloseWithError(io.EOF)
	s.reader.CloseWithError(io.EOF)
	return nil
}

func (s *AudioChannelSide) Read(p []byte) (n int, err error) {
	return s.reader.Read(p)
}

func (s *AudioChannelSide) Write(p []byte) (n int, err error) {
	return s.writer.Write(p)
}

func (s *AudioChannelSide) ClearReadBuffer() (int, error) {
	n := s.reader.Length()
	s.reader.Reset()
	return n, nil
}

func NewAudioChannel(audioBufferSize int) (*AudioChannelSide, *AudioChannelSide) {
	transportBuffer := ringbuffer.New(audioBufferSize).SetBlocking(true)
	sessionBuffer := ringbuffer.New(audioBufferSize).SetBlocking(true)

	// TODO: with cancel

	transportRW := &AudioChannelSide{
		reader: transportBuffer,
		writer: sessionBuffer,
	}

	sessionRw := &AudioChannelSide{
		reader: sessionBuffer,
		writer: transportBuffer,
	}

	return sessionRw, transportRW
}
