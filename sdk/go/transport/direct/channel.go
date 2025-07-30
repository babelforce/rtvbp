package direct

import (
	"context"
	"fmt"
	"sync"

	"github.com/babelforce/rtvbp-go"
)

type closer struct {
	c         chan struct{}
	closeOnce sync.Once
}

func (c *closer) close() {
	c.closeOnce.Do(func() {
		close(c.c)
	})
}

type myDirectTransport struct {
	a chan rtvbp.DataPackage
	b chan rtvbp.DataPackage
	c *closer
}

func (d *myDirectTransport) Close(_ context.Context) error {
	d.c.close()
	return nil
}

func (d *myDirectTransport) Write(data []byte) error {
	select {
	case <-d.c.c:
		return fmt.Errorf("transport is closed")
	case d.a <- rtvbp.DataPackage{Data: data}:
	}
	return nil
}

func (d *myDirectTransport) ReadChan() <-chan rtvbp.DataPackage {
	return d.b
}

var _ rtvbp.Transport = &myDirectTransport{}

func New() (rtvbp.Transport, rtvbp.Transport) {
	var (
		a = make(chan rtvbp.DataPackage, 1)
		b = make(chan rtvbp.DataPackage, 1)
		c = &closer{
			c: make(chan struct{}),
		}
	)

	t1 := &myDirectTransport{
		a: a,
		b: b,
		c: c,
	}

	t2 := &myDirectTransport{
		a: b,
		b: a,
		c: c,
	}

	return t1, t2
}

func NewTestSessions(h1 rtvbp.SessionHandler, h2 rtvbp.SessionHandler) (*rtvbp.Session, *rtvbp.Session) {
	var (
		t1, t2 = New()
		s1     = rtvbp.NewSession(rtvbp.WithTransport(t1), rtvbp.WithHandler(h1))
		s2     = rtvbp.NewSession(rtvbp.WithTransport(t2), rtvbp.WithHandler(h2))
	)

	return s1, s2
}
