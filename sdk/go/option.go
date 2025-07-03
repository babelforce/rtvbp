package rtvbp

import (
	"github.com/babelforce/rtvbp-go/proto"
	"log/slog"
)

type sessionOptions struct {
	id              string
	logger          *slog.Logger
	transport       TransportFunc
	handler         SessionHandler
	audioBufferSize int
}

type Option func(opts *sessionOptions)

func withDefaults() Option {
	return withOptions(
		WithLogger(slog.Default()),
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

func WithTransport(f TransportFunc) Option {
	return func(opts *sessionOptions) {
		opts.transport = f
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
