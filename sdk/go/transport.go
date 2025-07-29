package rtvbp

import (
	"context"
	"io"
)

type DataPackage struct {
	Data       []byte
	ReceivedAt int64
}

type DataChannel interface {
	Write(data []byte) error
	ReadChan() <-chan DataPackage
}

type Transport interface {
	Closed() <-chan struct{}
	Control() DataChannel
	Close(ctx context.Context) error
}

type TransportFactory func(ctx context.Context, audio io.ReadWriter) (Transport, error)
