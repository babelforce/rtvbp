package rtvbp

import (
	"context"
	"io"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go/internal/idgen"
)

type sessionOptions struct {
	id              string
	logger          *slog.Logger
	transport       LegacyTransportFactory
	handler         SessionHandler
	audioBufferSize int
	requestTimeout  time.Duration
	debug           bool
	streamObserver  *AudioStreamObserver
}

type Option func(opts *sessionOptions)

func withDefaults() Option {
	return withOptions(
		WithLogger(slog.Default()),
		WithRequestTimeout(5*time.Second),
		WithAudioBufferSize(1024*1024),
		WithID(idgen.ID()),
	)
}

func withOptions(os ...Option) Option {
	return func(opts *sessionOptions) {
		for _, o := range os {
			o(opts)
		}
	}
}

func WithStreamObserver(o AudioStreamObserver) Option {
	return func(opts *sessionOptions) {
		opts.streamObserver = &o
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

// WithTransportFactory configures the imported byte-oriented session runtime.
//
// Deprecated: R-9 migrates Session to TransportFactory and the semantic transport interfaces.
func WithTransportFactory(f LegacyTransportFactory) Option {
	return func(opts *sessionOptions) {
		opts.transport = f
	}
}

// WithTransport configures the imported byte-oriented session runtime.
//
// Deprecated: R-9 migrates Session to Transport and the semantic transport interfaces.
func WithTransport(t LegacyTransport) Option {
	return func(opts *sessionOptions) {
		opts.transport = func(ctx context.Context, audio io.ReadWriter) (LegacyTransport, error) {
			return t, nil
		}
	}
}

func WithHandler(h SessionHandler) Option {
	return func(opts *sessionOptions) {
		opts.handler = h
	}
}

func WithDebug(debug bool) Option {
	return func(opts *sessionOptions) {
		opts.debug = debug
	}
}

func WithAudioBufferSize(size int) Option {
	return func(opts *sessionOptions) {
		opts.audioBufferSize = size
	}
}
