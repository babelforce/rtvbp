package audio

import (
	"io"
	"sync"
)

type peekableReader interface {
	io.Reader
	Len() int
}

// NonBlockingReader wraps an io.Reader and makes it non-blocking
type NonBlockingReader struct {
	src peekableReader
	mu  sync.Mutex
}

func NewNonBlockingReader(r peekableReader) *NonBlockingReader {
	return &NonBlockingReader{src: r}
}

// Read reads from the underlying reader without blocking.
// If no data is immediately available, it returns 0, nil.
func (n *NonBlockingReader) Read(p []byte) (int, error) {
	n.mu.Lock()
	defer n.mu.Unlock()
	if n.src.Len() == 0 {
		return 0, nil // no data available, return non-blocking
	}
	return n.src.Read(p)
}
