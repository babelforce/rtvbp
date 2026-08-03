package rtvbp

import (
	"context"
	"errors"
	"io"
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

// TransportFactory creates a transport. The envelope is supplied so composite transports can
// exchange reserved transport.* signaling without coupling the transport to a specific codec.
type TransportFactory func(ctx context.Context, envelope Envelope) (Transport, error)

// DataPackage is the received-byte shape used by the imported legacy session runtime.
//
// Deprecated: R-9 migrates Session to ControlFrame and the new Transport interface.
type DataPackage struct {
	Data       []byte
	ReceivedAt int64
}

// LegacyTransport is today's byte-oriented transport contract retained only until R-9.
//
// Deprecated: implement Transport for new bindings.
type LegacyTransport interface {
	Write(data []byte) error
	ReadChan() <-chan DataPackage
	Close(ctx context.Context) error
}

// LegacyTransportFactory creates the byte-oriented transport consumed by today's Session.
//
// Deprecated: R-9 replaces this with TransportFactory.
type LegacyTransportFactory func(ctx context.Context, audio io.ReadWriter) (LegacyTransport, error)
