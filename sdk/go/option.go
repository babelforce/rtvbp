package rtvbp

import (
	"context"
	"io"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp-go/proto"
)

type sessionOptions struct {
	id              string
	logger          *slog.Logger
	transport       TransportFactory
	handler         SessionHandler
	audioBufferSize int
	requestTimeout  time.Duration
}

type Option func(opts *sessionOptions)

func withDefaults() Option {
	return withOptions(
		WithLogger(slog.Default()),
		WithRequestTimeout(5*time.Second),
		WithAudioBufferSize(1024*1024),
		WithID(proto.ID()),
	)
}

func withOptions(os ...Option) Option {
	return func(opts *sessionOptions) {
		for _, o := range os {
			o(opts)
		}
	}
}

func WithRequestTimeout(timeout time.Duration) Option {
	return func(opts *sessionOptions) {
		opts.requestTimeout = timeout
	}
}

func WithLogger(logger *slog.Logger) Option {
	return func(opts *sessionOptions) {
		opts.logger = logger
	}
}

func WithID(id string) Option {
	return func(opts *sessionOptions) {
		opts.id = id
	}
}

func WithTransportFactory(f TransportFactory) Option {
	return func(opts *sessionOptions) {
		opts.transport = f
	}
}

func WithTransport(t Transport) Option {
	return func(opts *sessionOptions) {
		opts.transport = func(ctx context.Context, audio io.ReadWriter) (Transport, error) {
			return t, nil
		}
	}
}

func WithHandler(h SessionHandler) Option {
	return func(opts *sessionOptions) {
		opts.handler = h
	}
}

func WithAudioBufferSize(size int) Option {
	return func(opts *sessionOptions) {
		opts.audioBufferSize = size
	}
}
