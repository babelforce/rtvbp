package direct

import (
	"bytes"
	"context"
	"github.com/babelforce/rtvbp-go"
	"sync"
)

type dcc struct {
	in     chan []byte
	out    chan []byte
	closed chan struct{}
	once   sync.Once
}

func (d *dcc) WriteChan() chan<- []byte {
	return d.out
}

func (d *dcc) ReadChan() <-chan []byte {
	return d.in
}

func (d *dcc) Close(_ context.Context) error {
	d.once.Do(func() {
		close(d.closed)
	})
	return nil
}

var _ rtvbp.DataChannel = &dcc{}

type directTransport struct {
	cc    *dcc
	audio bytes.Buffer
}

func (d *directTransport) Read(p []byte) (n int, err error) {
	return d.audio.Read(p)
}

func (d *directTransport) Write(p []byte) (n int, err error) {
	return d.audio.Write(p)
}

func (d *directTransport) Closed() <-chan struct{} {
	return make(chan struct{}, 1)
}

func (d *directTransport) Close(ctx context.Context) error {
	return nil
}

func (d *directTransport) Control() rtvbp.DataChannel {
	return d.cc
}

func newTransports() (rtvbp.Transport, rtvbp.Transport) {
	aToB := make(chan []byte, 32)
	bToA := make(chan []byte, 32)

	a := &directTransport{
		cc: &dcc{
			in:     bToA,
			out:    aToB,
			closed: make(chan struct{}),
		},
	}

	b := &directTransport{
		cc: &dcc{
			in:     aToB,
			out:    bToA,
			closed: make(chan struct{}),
		},
	}
	return a, b
}
