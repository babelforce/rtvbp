package rtvbp

import (
	"context"
	"io"
)

type DataChannel interface {
	WriteChan() chan<- []byte
	ReadChan() <-chan []byte
}

type Transport interface {
	io.ReadWriter
	Closed() <-chan struct{}
	Control() DataChannel
	Close(ctx context.Context) error
}

type Acceptor interface {
	Run(ctx context.Context) error
	Channel() <-chan Transport
	Shutdown(ctx context.Context) error
}
