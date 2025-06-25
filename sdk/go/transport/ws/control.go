package ws

import (
	"babelforce.go/ivr/rtvbp/rtvbp-go"
	"context"
	"sync"
)

type controlChannel struct {
	input  chan []byte
	output chan []byte
	once   sync.Once
	closed chan struct{}
}

func (cc *controlChannel) WriteChan() chan<- []byte {
	return cc.output
}

func (cc *controlChannel) ReadChan() <-chan []byte {
	return cc.input
}

func (cc *controlChannel) Close(_ context.Context) error {
	cc.once.Do(func() {
		close(cc.closed)
	})
	return nil
}

func newControlChannel() *controlChannel {
	input := make(chan []byte, 1)
	output := make(chan []byte, 1)

	return &controlChannel{
		input:  input,
		output: output,
		closed: make(chan struct{}, 1),
	}
}

var _ rtvbp.DataChannel = &controlChannel{}
