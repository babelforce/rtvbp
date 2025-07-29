package rtvbp

import (
	"io"

	"github.com/smallnest/ringbuffer"
)

type readWriter struct {
	io.Reader
	io.Writer
}

type handlerAudio struct {
	io.ReadWriter
	buf *ringbuffer.RingBuffer
}

func (s *handlerAudio) ClearBuffer() (int, error) {
	n := s.buf.Length()
	s.buf.Reset()
	return n, nil
}

func (s *handlerAudio) Len() int {
	return s.buf.Length()
}

type DuplexAudio struct {
	toTransportBuffer *ringbuffer.RingBuffer
	toSessionBuffer   *ringbuffer.RingBuffer
}

func (s *DuplexAudio) toHandlerAudio() HandlerAudio {
	return &handlerAudio{
		ReadWriter: s.SessionRW(),
		buf:        s.toSessionBuffer,
	}
}

func (s *DuplexAudio) TransportRW() io.ReadWriter {
	return &readWriter{
		Reader: s.toTransportBuffer,
		Writer: s.toSessionBuffer,
	}
}

func (s *DuplexAudio) SessionRW() io.ReadWriter {
	return &readWriter{
		Reader: s.toSessionBuffer,
		Writer: s.toTransportBuffer,
	}
}

func NewSessionAudio(audioBufferSize int) *DuplexAudio {
	return &DuplexAudio{
		toTransportBuffer: ringbuffer.New(audioBufferSize).SetBlocking(true),
		toSessionBuffer:   ringbuffer.New(audioBufferSize).SetBlocking(true),
	}
}
