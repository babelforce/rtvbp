package rtvbp

import (
	"context"
	"errors"
	"time"
)

// ErrMediaUnsupported reports that a transport cannot open dynamic media.
var ErrMediaUnsupported = errors.New("rtvbp: dynamic media is unsupported")

// Received is one opaque control message received from a transport.
type Received struct {
	Data       []byte
	ReceivedAt time.Time
}

// ControlChannel carries opaque envelope bytes. Transports do not inspect methods or identifiers.
// Recv returns io.EOF after an orderly close and after all admitted messages have been received.
type ControlChannel interface {
	Send(ctx context.Context, data []byte) error
	Recv(ctx context.Context) (Received, error)
}

// MediaFormat describes the encoding and packet cadence of a media channel.
type MediaFormat struct {
	Encoding   string
	SampleRate int
	BitDepth   int
	Channels   int
	PTime      time.Duration
}

// MediaFrame is one transport media frame. PTS is meaningful only when Timed is true.
type MediaFrame struct {
	Data  []byte
	PTS   time.Duration
	Timed bool
}

// MediaChannel is one named duplex media stream. "audio" is the default duplex voice stream ID.
type MediaChannel interface {
	ID() string
	Format() MediaFormat
	WriteFrame(MediaFrame) error
	ReadFrame() (MediaFrame, error)
	Close() error
}

// Transport carries one control channel and zero or more media channels.
//
// Close must flush every queued control send before tearing down the underlying connection.
// The transport.* control-method namespace is reserved for transport signaling and cannot be
// claimed by a payload catalog.
type Transport interface {
	Control() ControlChannel
	AcceptMedia(ctx context.Context) (MediaChannel, error)
	OpenMedia(ctx context.Context, id string, format MediaFormat) (MediaChannel, error)
	Close(ctx context.Context) error
}

// TransportFactory creates a transport. Its context bounds construction only; Session owns and
// explicitly closes a successfully returned transport. Implementations must stop construction
// promptly when the context is canceled and must not use that context as the transport lifetime.
// The envelope is supplied so composite transports can exchange reserved transport.* signaling
// without coupling the transport to a specific codec.
type TransportFactory func(ctx context.Context, envelope Envelope) (Transport, error)
