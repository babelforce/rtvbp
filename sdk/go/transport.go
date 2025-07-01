package rtvbp

import (
	"context"
	"io"
)

type DataChannel interface {
	Write(data []byte) error
	ReadChan() <-chan []byte
}

type Transport interface {
	io.ReadWriter
	Closed() <-chan struct{}
	Control() DataChannel
	Close(ctx context.Context) error
}
