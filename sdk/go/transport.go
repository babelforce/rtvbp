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
	Closed() <-chan struct{}
	Control() DataChannel
	Close(ctx context.Context) error
}

type TransportFunc func(ctx context.Context, audio io.ReadWriter) (Transport, error)
