package direct

import (
	"bytes"
	"context"
	"sync"

	"github.com/babelforce/rtvbp-go"
)

type dcc struct {
	in     chan rtvbp.DataPackage
	out    chan rtvbp.DataPackage
	closed chan struct{}
	once   sync.Once
}

func (d *dcc) Write(data []byte) error {
	d.out <- rtvbp.DataPackage{Data: data}
	return nil
}

func (d *dcc) ReadChan() <-chan rtvbp.DataPackage {
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

func New() (rtvbp.Transport, rtvbp.Transport) {
	aToB := make(chan rtvbp.DataPackage, 1)
	bToA := make(chan rtvbp.DataPackage, 1)

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

func NewTestSessions(h1 rtvbp.SessionHandler, h2 rtvbp.SessionHandler) (*rtvbp.Session, *rtvbp.Session) {
	var (
		t1, t2 = New()
		s1     = rtvbp.NewSession(rtvbp.WithTransport(t1), rtvbp.WithHandler(h1))
		s2     = rtvbp.NewSession(rtvbp.WithTransport(t2), rtvbp.WithHandler(h2))
	)

	return s1, s2
}
