package direct

import (
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

func (d *dcc) Output() chan<- []byte {
	return d.out
}

func (d *dcc) Input() <-chan []byte {
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
	cc *dcc
}

func (d *directTransport) Audio() rtvbp.AudioChannel {
	//TODO implement me
	panic("implement me")
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
