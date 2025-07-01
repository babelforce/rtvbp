package audio

import (
	"github.com/smallnest/ringbuffer"
	"io"
	"sync"
)

type Buffer struct {
	b              *ringbuffer.RingBuffer
	bytesPerSample int
	sampleRate     int
	mu             sync.Mutex
	cond           *sync.Cond
	closed         bool
}

func (buf *Buffer) Read(p []byte) (int, error) {
	buf.mu.Lock()
	defer buf.mu.Unlock()

	for buf.b.Length() < len(p) && !buf.closed {
		buf.cond.Wait()
	}

	if buf.closed && buf.b.Length() < len(p) {
		return 0, io.EOF
	}

	n := min(len(p), buf.b.Length())
	if n > 0 {
		_, _ = buf.b.Read(p[:n])
	}

	return n, nil
}

func (buf *Buffer) Write(p []byte) (int, error) {
	buf.mu.Lock()
	defer buf.mu.Unlock()

	if buf.closed {
		return 0, io.ErrClosedPipe
	}

	n, err := buf.b.Write(p)
	if n > 0 {
		buf.cond.Broadcast()
	}
	return n, err
}

func (buf *Buffer) Close() error {
	buf.mu.Lock()
	defer buf.mu.Unlock()
	buf.closed = true
	buf.cond.Broadcast()
	return nil
}

func NewBuffer(size int) *Buffer {
	b := &Buffer{
		b: ringbuffer.New(size).SetBlocking(true),
	}
	b.cond = sync.NewCond(&b.mu)
	return b
}
