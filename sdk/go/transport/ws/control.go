package ws

import (
	"fmt"
	"sync"

	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
)

type controlChannel struct {
	mu        sync.Mutex
	closed    bool
	input     chan rtvbp.DataPackage
	output    chan<- wsMessage
	closeOnce sync.Once
}

func (cc *controlChannel) writeIn(p rtvbp.DataPackage) error {
	cc.mu.Lock()
	defer cc.mu.Unlock()
	if cc.closed {
		return fmt.Errorf("control channel is closed")
	}
	cc.input <- p
	return nil
}

func (cc *controlChannel) writeOut(data []byte) error {
	cc.mu.Lock()
	defer cc.mu.Unlock()

	if cc.closed {
		return fmt.Errorf("control channel is closed")
	}
	cc.output <- wsMessage{mt: websocket.TextMessage, data: data}
	return nil
}

func (cc *controlChannel) readChan() <-chan rtvbp.DataPackage {
	return cc.input
}

func (cc *controlChannel) close() {
	cc.closeOnce.Do(func() {
		cc.mu.Lock()
		defer cc.mu.Unlock()

		cc.closed = true
		close(cc.input)
	})
}

func newControlChannel(output chan<- wsMessage) *controlChannel {
	input := make(chan rtvbp.DataPackage, 16)
	return &controlChannel{
		input:  input,
		output: output,
	}
}
