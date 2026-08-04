package webrtcws

import (
	"context"
	"errors"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/pion/webrtc/v4"
	"github.com/pion/webrtc/v4/pkg/media"
)

const (
	audioID       = "audio"
	pcmuClockRate = 8_000
	pcmuPTime     = 20 * time.Millisecond
)

var (
	errMediaClaimed = errors.New("webrtcws: audio media has already been opened or accepted")
	errPeerFailed   = errors.New("webrtcws: WebRTC peer connection failed")
)

type frameResult struct {
	frame rtvbp.MediaFrame
	err   error
}

type mediaChannel struct {
	track *webrtc.TrackLocalStaticSample

	mu          sync.Mutex
	format      rtvbp.MediaFormat
	closed      bool
	terminalErr error
	inbound     chan frameResult
	done        chan struct{}
	once        sync.Once
}

func newMediaChannel(track *webrtc.TrackLocalStaticSample, format rtvbp.MediaFormat) *mediaChannel {
	return &mediaChannel{
		track:   track,
		format:  format,
		inbound: make(chan frameResult, 128),
		done:    make(chan struct{}),
	}
}

func (m *mediaChannel) ID() string { return audioID }

func (m *mediaChannel) Format() rtvbp.MediaFormat {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.format
}

func (m *mediaChannel) configure(format rtvbp.MediaFormat) error {
	if err := validateFormat(format); err != nil {
		return err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.closed {
		return io.EOF
	}
	if m.format != (rtvbp.MediaFormat{}) && m.format != format {
		return fmt.Errorf("webrtcws: audio format already configured as %#v", m.format)
	}
	m.format = format
	return nil
}

func (m *mediaChannel) WriteFrame(frame rtvbp.MediaFrame) error {
	m.mu.Lock()
	closed := m.closed
	format := m.format
	m.mu.Unlock()
	if closed {
		return io.ErrClosedPipe
	}
	frameBytes, err := format.FrameBytes()
	if err != nil {
		return fmt.Errorf("webrtcws: audio format is not configured: %w", err)
	}
	if len(frame.Data) != frameBytes {
		return fmt.Errorf("webrtcws: L16 frame has %d bytes, want %d", len(frame.Data), frameBytes)
	}
	return m.track.WriteSample(media.Sample{Data: encodePCMU(frame.Data), Duration: format.PTime})
}

func (m *mediaChannel) ReadFrame() (rtvbp.MediaFrame, error) {
	select {
	case result := <-m.inbound:
		return result.frame, result.err
	default:
	}
	select {
	case result := <-m.inbound:
		return result.frame, result.err
	case <-m.done:
		select {
		case result := <-m.inbound:
			return result.frame, result.err
		default:
			m.mu.Lock()
			err := m.terminalErr
			m.mu.Unlock()
			return rtvbp.MediaFrame{}, err
		}
	}
}

func (m *mediaChannel) Close() error {
	m.fail(io.EOF)
	return nil
}

func (m *mediaChannel) fail(err error) {
	if err == nil {
		err = io.EOF
	}
	m.once.Do(func() {
		m.mu.Lock()
		m.closed = true
		m.terminalErr = err
		m.mu.Unlock()
		close(m.done)
	})
}

func (m *mediaChannel) receive(track *webrtc.TrackRemote) {
	var firstTimestamp uint32
	haveFirst := false
	for {
		packet, _, err := track.ReadRTP()
		if err != nil {
			m.fail(normalizeMediaError(err))
			return
		}
		if !haveFirst {
			firstTimestamp = packet.Timestamp
			haveFirst = true
		}
		pts := time.Duration(packet.Timestamp-firstTimestamp) * time.Second / pcmuClockRate
		result := frameResult{frame: rtvbp.MediaFrame{
			Data:  decodePCMU(packet.Payload),
			PTS:   pts,
			Timed: true,
		}}
		select {
		case m.inbound <- result:
		case <-m.done:
			return
		}
	}
}

func normalizeMediaError(err error) error {
	if err == nil || errors.Is(err, io.EOF) {
		return io.EOF
	}
	return fmt.Errorf("webrtcws: read RTP audio: %w", err)
}

func validateFormat(format rtvbp.MediaFormat) error {
	if format.Encoding != "L16" || format.SampleRate != pcmuClockRate || format.BitDepth != 16 || format.Channels != 1 || format.PTime != pcmuPTime {
		return fmt.Errorf("webrtcws: unsupported audio format %#v; want L16/8000/16-bit/mono/20ms", format)
	}
	_, err := format.FrameBytes()
	return err
}

func waitConnected(ctx context.Context, connected <-chan error) error {
	select {
	case err := <-connected:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}
