package audio

import "io"

type DuplexEndpoint struct {
	read  <-chan []byte // read is a channel for incoming data
	write chan<- []byte // write is a channel for outgoing data
}

func (p *DuplexEndpoint) ReadChan() <-chan []byte {
	return p.read
}

func (p *DuplexEndpoint) WriteChan() chan<- []byte {
	return p.write
}

func (p *DuplexEndpoint) ToNonBlockingRW() io.ReadWriter {
	return NewNonBlockingRW(p)
}

func DuplexPipe(done <-chan struct{}, e1 AudioIO, e2 AudioIO) {
	go pipe(done, e1.ReadChan(), e2.WriteChan())
	go pipe(done, e2.ReadChan(), e1.WriteChan())
}

func pipe(done <-chan struct{}, in <-chan []byte, out chan<- []byte) {
	for {
		select {
		case <-done:
			return
		case data, ok := <-in:
			if !ok {
				return
			}

			// write
			select {
			case <-done:
				return
			case out <- data:
			}
		}
	}
}

func NewDuplex(done <-chan struct{}, n int) (*DuplexEndpoint, *DuplexEndpoint) {
	c1 := make(chan []byte, n)
	c2 := make(chan []byte, n)

	e1 := &DuplexEndpoint{
		read:  c1,
		write: c2,
	}

	e2 := &DuplexEndpoint{
		read:  c2,
		write: c1,
	}

	DuplexPipe(done, e1, e2)

	return e1, e2
}
