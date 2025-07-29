package ws

import (
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
)

type controlChannel struct {
	input  chan rtvbp.DataPackage
	output chan<- wsMessage
}

func (cc *controlChannel) Write(data []byte) error {
	cc.output <- wsMessage{mt: websocket.TextMessage, data: data}
	return nil
}

func (cc *controlChannel) ReadChan() <-chan rtvbp.DataPackage {
	return cc.input
}

func newControlChannel(output chan<- wsMessage) *controlChannel {
	input := make(chan rtvbp.DataPackage, 16)

	return &controlChannel{
		input:  input,
		output: output,
	}
}

var _ rtvbp.DataChannel = &controlChannel{}
