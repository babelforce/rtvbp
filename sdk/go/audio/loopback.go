package audio

import (
	"bytes"
	"io"
	"sync"
)

type Loopback struct {
	mu     sync.Mutex
	buf    *bytes.Buffer
	cond   *sync.Cond
	closed bool
}

func NewLoopback() *Loopback {
	l := &Loopback{
		buf: new(bytes.Buffer),
	}
	l.cond = sync.NewCond(&l.mu)
	return l
}

func (l *Loopback) Write(p []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	if l.closed {
		return 0, io.ErrClosedPipe
	}
	n, err := l.buf.Write(p)
	l.cond.Signal() // wake any blocked readers
	return n, err
}

func (l *Loopback) Read(p []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()

	for l.buf.Len() == 0 && !l.closed {
		l.cond.Wait()
	}

	if l.buf.Len() == 0 && l.closed {
		return 0, io.EOF
	}

	return l.buf.Read(p)
}

func (l *Loopback) Close() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.closed = true
	l.cond.Broadcast()
	return nil
}
