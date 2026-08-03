package rtvbp

import (
	"context"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go/internal/idgen"
)

type IDGenerator func() string

type sessionOptions struct {
	id              string
	idGenerator     IDGenerator
	logger          *slog.Logger
	transport       TransportFactory
	handler         SessionHandler
	audioBufferSize int
	requestTimeout  time.Duration
	closeTimeout    time.Duration
	keepalive       KeepalivePolicy
	debug           bool
	streamObserver  *AudioStreamObserver
}

type Option func(*sessionOptions)

func withDefaults() Option {
	return withOptions(
		WithLogger(slog.Default()),
		WithRequestTimeout(5*time.Second),
		WithCloseTimeout(5*time.Second),
		WithAudioBufferSize(1024*1024),
		WithID(idgen.ID()),
		WithIDGenerator(idgen.ID),
	)
}

func withOptions(options ...Option) Option {
	return func(config *sessionOptions) {
		for _, option := range options {
			option(config)
		}
	}
}

func WithStreamObserver(observer AudioStreamObserver) Option {
	return func(options *sessionOptions) { options.streamObserver = &observer }
}

func WithRequestTimeout(timeout time.Duration) Option {
	return func(options *sessionOptions) { options.requestTimeout = timeout }
}

func WithCloseTimeout(timeout time.Duration) Option {
	return func(options *sessionOptions) { options.closeTimeout = timeout }
}

func WithKeepalivePolicy(policy KeepalivePolicy) Option {
	return func(options *sessionOptions) { options.keepalive = policy }
}

func WithLogger(logger *slog.Logger) Option {
	return func(options *sessionOptions) { options.logger = logger }
}

func WithID(id string) Option {
	return func(options *sessionOptions) { options.id = id }
}

func WithIDGenerator(generator IDGenerator) Option {
	return func(options *sessionOptions) { options.idGenerator = generator }
}

func WithTransportFactory(factory TransportFactory) Option {
	return func(options *sessionOptions) { options.transport = factory }
}

func WithTransport(transport Transport) Option {
	return WithTransportFactory(func(context.Context, Envelope) (Transport, error) {
		return transport, nil
	})
}

func WithHandler(handler SessionHandler) Option {
	return func(options *sessionOptions) { options.handler = handler }
}

func WithDebug(debug bool) Option {
	return func(options *sessionOptions) { options.debug = debug }
}

func WithAudioBufferSize(size int) Option {
	return func(options *sessionOptions) { options.audioBufferSize = size }
}
