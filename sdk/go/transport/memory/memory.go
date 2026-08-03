// Package memory provides an in-process RTVBP transport pair.
package memory

import (
	"context"
	"errors"
	"io"
	"sync"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
)

var errMediaAlreadyOpen = errors.New("memory transport: media channel already opened")

// Option configures a memory transport pair.
type Option func(*config)

type config struct {
	media bool
}

// WithMedia enables one dynamically opened in-process media channel.
func WithMedia() Option {
	return func(cfg *config) {
		cfg.media = true
	}
}

// Transport is one endpoint of an in-process transport pair.
type Transport struct {
	side    int
	pair    *pair
	control *controlChannel
}

// NewPair returns two connected in-process transports.
func NewPair(options ...Option) (*Transport, *Transport) {
	cfg := config{}
	for _, option := range options {
		option(&cfg)
	}

	control := [2]*mailbox[rtvbp.Received]{newMailbox[rtvbp.Received](), newMailbox[rtvbp.Received]()}
	p := &pair{
		mediaEnabled: cfg.media,
		control:      control,
		mediaReady:   [2]chan struct{}{make(chan struct{}, 1), make(chan struct{}, 1)},
	}

	first := &Transport{
		side:    0,
		pair:    p,
		control: &controlChannel{incoming: control[0], outgoing: control[1]},
	}
	second := &Transport{
		side:    1,
		pair:    p,
		control: &controlChannel{incoming: control[1], outgoing: control[0]},
	}
	return first, second
}

// Control returns the pair endpoint's control channel.
func (t *Transport) Control() rtvbp.ControlChannel {
	return t.control
}

// OpenMedia opens the pair's sole optional media channel.
func (t *Transport) OpenMedia(ctx context.Context, id string, format rtvbp.MediaFormat) (rtvbp.MediaChannel, error) {
	return t.pair.openMedia(ctx, t.side, id, format)
}

// AcceptMedia waits for the peer to open the pair's optional media channel.
func (t *Transport) AcceptMedia(ctx context.Context) (rtvbp.MediaChannel, error) {
	return t.pair.acceptMedia(ctx, t.side)
}

// Close closes both endpoints. Control frames admitted before Close remain available to Recv.
func (t *Transport) Close(_ context.Context) error {
	t.pair.close()
	return nil
}

type controlChannel struct {
	incoming *mailbox[rtvbp.Received]
	outgoing *mailbox[rtvbp.Received]
}

func (c *controlChannel) Send(ctx context.Context, data []byte) error {
	return c.outgoing.push(ctx, rtvbp.Received{
		Data:       clone(data),
		ReceivedAt: time.Now(),
	})
}

func (c *controlChannel) Recv(ctx context.Context) (rtvbp.Received, error) {
	return c.incoming.pop(ctx)
}

type pair struct {
	mu            sync.Mutex
	closeOnce     sync.Once
	closed        bool
	mediaEnabled  bool
	mediaOpened   bool
	control       [2]*mailbox[rtvbp.Received]
	pendingMedia  [2]*mediaChannel
	mediaReady    [2]chan struct{}
	openMediaPair *mediaPair
}

func (p *pair) openMedia(ctx context.Context, side int, id string, format rtvbp.MediaFormat) (rtvbp.MediaChannel, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if !p.mediaEnabled {
		return nil, rtvbp.ErrMediaUnsupported
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if p.closed {
		return nil, io.ErrClosedPipe
	}
	if p.mediaOpened {
		return nil, errMediaAlreadyOpen
	}

	media := newMediaPair(id, format)
	p.mediaOpened = true
	p.openMediaPair = media
	p.pendingMedia[1-side] = media.channels[1-side]
	signal(p.mediaReady[1-side])
	return media.channels[side], nil
}

func (p *pair) acceptMedia(ctx context.Context, side int) (rtvbp.MediaChannel, error) {
	p.mu.Lock()
	if !p.mediaEnabled {
		p.mu.Unlock()
		return nil, rtvbp.ErrMediaUnsupported
	}

	for {
		if err := ctx.Err(); err != nil {
			p.mu.Unlock()
			return nil, err
		}
		if p.closed {
			p.mu.Unlock()
			return nil, io.EOF
		}
		if media := p.pendingMedia[side]; media != nil {
			p.pendingMedia[side] = nil
			p.mu.Unlock()
			return media, nil
		}

		ready := p.mediaReady[side]
		p.mu.Unlock()
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-ready:
		}
		p.mu.Lock()
	}
}

func (p *pair) close() {
	p.closeOnce.Do(func() {
		p.mu.Lock()
		p.closed = true
		media := p.openMediaPair
		p.mu.Unlock()

		p.control[0].close()
		p.control[1].close()
		signal(p.mediaReady[0])
		signal(p.mediaReady[1])
		if media != nil {
			media.close()
		}
	})
}

type mediaPair struct {
	closeOnce sync.Once
	mailboxes [2]*mailbox[rtvbp.MediaFrame]
	channels  [2]*mediaChannel
}

func newMediaPair(id string, format rtvbp.MediaFormat) *mediaPair {
	media := &mediaPair{
		mailboxes: [2]*mailbox[rtvbp.MediaFrame]{newMailbox[rtvbp.MediaFrame](), newMailbox[rtvbp.MediaFrame]()},
	}
	media.channels = [2]*mediaChannel{
		{id: id, format: format, incoming: media.mailboxes[0], outgoing: media.mailboxes[1], pair: media},
		{id: id, format: format, incoming: media.mailboxes[1], outgoing: media.mailboxes[0], pair: media},
	}
	return media
}

func (p *mediaPair) close() {
	p.closeOnce.Do(func() {
		p.mailboxes[0].close()
		p.mailboxes[1].close()
	})
}

type mediaChannel struct {
	id       string
	format   rtvbp.MediaFormat
	incoming *mailbox[rtvbp.MediaFrame]
	outgoing *mailbox[rtvbp.MediaFrame]
	pair     *mediaPair
}

func (m *mediaChannel) ID() string {
	return m.id
}

func (m *mediaChannel) Format() rtvbp.MediaFormat {
	return m.format
}

func (m *mediaChannel) WriteFrame(frame rtvbp.MediaFrame) error {
	frame.Data = clone(frame.Data)
	return m.outgoing.push(context.Background(), frame)
}

func (m *mediaChannel) ReadFrame() (rtvbp.MediaFrame, error) {
	return m.incoming.pop(context.Background())
}

func (m *mediaChannel) Close() error {
	m.pair.close()
	return nil
}

type mailbox[T any] struct {
	mu     sync.Mutex
	items  []T
	closed bool
	ready  chan struct{}
}

func newMailbox[T any]() *mailbox[T] {
	return &mailbox[T]{ready: make(chan struct{}, 1)}
}

func (m *mailbox[T]) push(ctx context.Context, item T) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if err := ctx.Err(); err != nil {
		return err
	}
	if m.closed {
		return io.ErrClosedPipe
	}
	m.items = append(m.items, item)
	signal(m.ready)
	return nil
}

func (m *mailbox[T]) pop(ctx context.Context) (T, error) {
	var zero T
	for {
		if err := ctx.Err(); err != nil {
			return zero, err
		}

		m.mu.Lock()
		if len(m.items) != 0 {
			item := m.items[0]
			m.items[0] = zero
			m.items = m.items[1:]
			m.mu.Unlock()
			return item, nil
		}
		if m.closed {
			m.mu.Unlock()
			return zero, io.EOF
		}
		ready := m.ready
		m.mu.Unlock()

		select {
		case <-ctx.Done():
			return zero, ctx.Err()
		case <-ready:
		}
	}
}

func (m *mailbox[T]) close() {
	m.mu.Lock()
	m.closed = true
	m.mu.Unlock()
	signal(m.ready)
}

func signal(ch chan struct{}) {
	select {
	case ch <- struct{}{}:
	default:
	}
}

func clone(data []byte) []byte {
	if data == nil {
		return nil
	}
	return append([]byte(nil), data...)
}

var _ rtvbp.Transport = (*Transport)(nil)
var _ rtvbp.ControlChannel = (*controlChannel)(nil)
var _ rtvbp.MediaChannel = (*mediaChannel)(nil)
