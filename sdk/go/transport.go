package rtvbp

import (
	"context"
	"io"
)

type DataPackage struct {
	Data       []byte
	ReceivedAt int64
}

type Transport interface {
	Write(data []byte) error
	ReadChan() <-chan DataPackage
	Close(ctx context.Context) error
}

type TransportFactory func(ctx context.Context, audio io.ReadWriter) (Transport, error)
