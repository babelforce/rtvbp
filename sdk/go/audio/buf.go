package audio

import (
	"io"
	"sync/atomic"
)

type DuplexBuffer struct {
	reader io.Reader
	writeC chan []byte
	len    atomic.Int64
}

func (d *DuplexBuffer) Len() int {
	return int(d.len.Load())
}

func (d *DuplexBuffer) Read(p []byte) (n int, err error) {
	n, err = d.reader.Read(p)
	d.len.Add(-int64(n))
	return n, err
}

// Write sends data without blocking by using an internal buffered channel
func (d *DuplexBuffer) Write(p []byte) (int, error) {

	data := make([]byte, len(p))
	copy(data, p)
	select {
	case d.writeC <- data:
		d.len.Add(int64(len(p)))
		return len(p), nil
	default:
		// Optional: drop data or block here instead if channel is full
		return 0, io.ErrShortWrite
	}
}

func NewDuplexBuffers() (*DuplexBuffer, *DuplexBuffer) {
	// Pipe A -> B
	paR, paW := io.Pipe()
	// Pipe B -> A
	pbR, pbW := io.Pipe()

	aToBChan := make(chan []byte, 4096)
	bToAChan := make(chan []byte, 4096)

	// Writer goroutines
	go func() {
		for data := range aToBChan {
			if _, err := paW.Write(data); err != nil {
				return
			}
		}
	}()
	go func() {
		for data := range bToAChan {
			if _, err := pbW.Write(data); err != nil {
				return
			}
		}
	}()

	endA := &DuplexBuffer{
		reader: pbR, // reads what B writes
		writeC: aToBChan,
	}
	endB := &DuplexBuffer{
		reader: paR, // reads what A writes
		writeC: bToAChan,
	}

	return endA, endB
}

var _ io.ReadWriter = &DuplexBuffer{}

func DuplexCopy(a, b io.ReadWriter) {
	go io.Copy(a, b) // b → a
	go io.Copy(b, a) // a → b
}
