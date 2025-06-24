package rtvbp

import (
	"context"
	"github.com/babelforce/rtvbp-go/audio"
)

type DataChannel interface {
	Output() chan<- []byte
	Input() <-chan []byte
}

type Transport interface {
	AudioIO() audio.AudioIO
	Closed() <-chan struct{}
	Control() DataChannel
	Close(ctx context.Context) error
}

type Acceptor interface {
	Run(ctx context.Context) error
	Channel() <-chan Transport
	Shutdown(ctx context.Context) error
}
