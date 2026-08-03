package ws

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"sync"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/gorilla/websocket"
)

const staticAudioID = "audio"

var errMediaFormatConflict = errors.New("websocket transport: audio media format already configured")

// Transport is the semantic RTVBP transport over one WebSocket connection.
// Text messages carry control frames and binary messages carry the static audio stream.
type Transport struct {
	conn   *websocket.Conn
	logger *slog.Logger

	wireSubprotocol      string
	effectiveSubprotocol string

	ctx    context.Context
	cancel context.CancelFunc
	done   chan struct{}

	control  *semanticControlChannel
	media    *staticMediaChannel
	outgoing *outboundQueue
	pongs    chan string

	writeMu    sync.Mutex
	monitorMu  sync.Mutex
	pingSerial uint64

	finishOnce sync.Once
	closeOnce  sync.Once
	closeAck   chan error

	errMu       sync.Mutex
	terminalErr error
}

// NewTransport starts a semantic transport over an already upgraded WebSocket.
func NewTransport(ctx context.Context, conn *websocket.Conn, config *TransportConfig) *Transport {
	logger := slog.Default()
	if config != nil && config.Logger != nil {
		logger = config.Logger
	}
	transportCtx, cancel := context.WithCancel(context.Background())
	t := &Transport{
		conn:                 conn,
		logger:               logger,
		wireSubprotocol:      conn.Subprotocol(),
		effectiveSubprotocol: effectiveSubprotocol(conn.Subprotocol()),
		ctx:                  transportCtx,
		cancel:               cancel,
		done:                 make(chan struct{}),
		outgoing:             newOutboundQueue(),
		pongs:                make(chan string, 64),
		closeAck:             make(chan error, 1),
	}
	t.control = &semanticControlChannel{transport: t, incoming: newInbox[rtvbp.Received]()}
	t.media = &staticMediaChannel{transport: t, incoming: newInbox[rtvbp.MediaFrame]()}
	conn.SetPongHandler(func(payload string) error {
		select {
		case t.pongs <- payload:
		default:
		}
		return nil
	})

	go t.readPump()
	go t.writePump()
	go func() {
		select {
		case <-ctx.Done():
			t.finish(ctx.Err())
		case <-t.done:
		}
	}()
	return t
}

// MonitorKeepalive monitors the connection using native WebSocket Ping/Pong
// control frames. It never emits a catalog-level text ping.
func (t *Transport) MonitorKeepalive(ctx context.Context, policy rtvbp.KeepalivePolicy) error {
	if err := policy.Validate(); err != nil {
		return err
	}
	if !policy.Enabled() {
		return nil
	}

	t.monitorMu.Lock()
	defer t.monitorMu.Unlock()

	misses := 0
	interval := time.NewTimer(policy.Interval)
	defer interval.Stop()
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-t.done:
			return keepaliveCloseResult(t.terminalError())
		case <-interval.C:
		}

		t.drainPongs()
		t.pingSerial++
		payload := fmt.Sprintf("rtvbp:%d", t.pingSerial)
		if err := t.enqueueAcknowledged(ctx, websocket.PingMessage, []byte(payload)); err != nil {
			return err
		}

		matched := false
		timeout := time.NewTimer(policy.Timeout)
	waitForPong:
		for {
			select {
			case <-ctx.Done():
				stopTimer(timeout)
				return ctx.Err()
			case <-t.done:
				stopTimer(timeout)
				return keepaliveCloseResult(t.terminalError())
			case pong := <-t.pongs:
				if pong == payload {
					matched = true
					stopTimer(timeout)
					break waitForPong
				}
			case <-timeout.C:
				break waitForPong
			}
		}

		if matched {
			misses = 0
		} else {
			misses++
			if misses >= policy.MaxMisses {
				t.finish(rtvbp.ErrKeepaliveTimeout)
				return rtvbp.ErrKeepaliveTimeout
			}
		}
		interval.Reset(policy.Interval)
	}
}

// Subprotocol returns the effective RTVBP profile. A peer that selects no
// wire subprotocol uses the backward-compatible rtvbp.v1 profile.
func (t *Transport) Subprotocol() string {
	return t.effectiveSubprotocol
}

// WireSubprotocol returns the value selected by the WebSocket handshake.
func (t *Transport) WireSubprotocol() string {
	return t.wireSubprotocol
}

// Control returns the text-message control channel.
func (t *Transport) Control() rtvbp.ControlChannel {
	return t.control
}

// OpenMedia returns the sole static audio channel.
func (t *Transport) OpenMedia(ctx context.Context, id string, format rtvbp.MediaFormat) (rtvbp.MediaChannel, error) {
	if id != staticAudioID {
		return nil, rtvbp.ErrMediaUnsupported
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if err := t.closedError(); err != nil {
		return nil, err
	}
	if err := t.media.configure(format); err != nil {
		return nil, err
	}
	return t.media, nil
}

// AcceptMedia returns the WebSocket's always-present static audio channel.
func (t *Transport) AcceptMedia(ctx context.Context) (rtvbp.MediaChannel, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if err := t.closedError(); err != nil {
		return nil, err
	}
	if t.media.isClosed() {
		return nil, io.EOF
	}
	return t.media, nil
}

// Close stops admission, flushes every admitted outbound message, and closes the socket.
func (t *Transport) Close(ctx context.Context) error {
	select {
	case <-t.done:
		return closeResult(t.terminalError())
	default:
	}

	t.closeOnce.Do(func() {
		command := outboundMessage{
			messageType: websocket.CloseMessage,
			data:        websocket.FormatCloseMessage(websocket.CloseNormalClosure, "Closed"),
			close:       true,
		}
		if !t.outgoing.closeWith(command) {
			t.closeAck <- t.terminalError()
		}
	})

	select {
	case err := <-t.closeAck:
		return closeResult(err)
	case <-t.done:
		return closeResult(t.terminalError())
	case <-ctx.Done():
		t.finish(ctx.Err())
		return fmt.Errorf("websocket close: %w", ctx.Err())
	}
}

func (t *Transport) readPump() {
	for {
		messageType, data, err := t.conn.ReadMessage()
		if err != nil {
			t.finish(normalizeSocketError(err))
			return
		}
		switch messageType {
		case websocket.TextMessage:
			_ = t.control.incoming.push(rtvbp.Received{
				Data:       cloneBytes(data),
				ReceivedAt: time.Now(),
			})
		case websocket.BinaryMessage:
			_ = t.media.incoming.push(rtvbp.MediaFrame{Data: cloneBytes(data)})
		}
	}
}

func (t *Transport) writePump() {
	for {
		message, err := t.outgoing.pop(t.ctx)
		if err != nil {
			return
		}
		err = t.writeMessage(message)
		if message.written != nil {
			message.written <- err
		}
		if message.close {
			if err == nil {
				t.finish(io.EOF)
			} else {
				t.finish(normalizeSocketError(err))
			}
			t.closeAck <- err
			return
		}
		if err != nil {
			t.finish(normalizeSocketError(err))
			return
		}
	}
}

func (t *Transport) writeMessage(message outboundMessage) error {
	t.writeMu.Lock()
	defer t.writeMu.Unlock()

	deadline := time.Now().Add(5 * time.Second)
	if message.messageType == websocket.CloseMessage || message.messageType == websocket.PingMessage || message.messageType == websocket.PongMessage {
		return t.conn.WriteControl(message.messageType, message.data, deadline)
	}
	if err := t.conn.SetWriteDeadline(deadline); err != nil {
		return err
	}
	return t.conn.WriteMessage(message.messageType, message.data)
}

func (t *Transport) enqueue(ctx context.Context, messageType int, data []byte) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := t.outgoing.push(outboundMessage{messageType: messageType, data: cloneBytes(data)}); err != nil {
		return err
	}
	return nil
}

func (t *Transport) enqueueAcknowledged(ctx context.Context, messageType int, data []byte) error {
	written := make(chan error, 1)
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := t.outgoing.push(outboundMessage{
		messageType: messageType,
		data:        cloneBytes(data),
		written:     written,
	}); err != nil {
		if errors.Is(err, io.ErrClosedPipe) {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-t.done:
				return keepaliveCloseResult(t.terminalError())
			}
		}
		return err
	}
	select {
	case err := <-written:
		return err
	case <-ctx.Done():
		return ctx.Err()
	case <-t.done:
		return keepaliveCloseResult(t.terminalError())
	}
}

func (t *Transport) drainPongs() {
	for {
		select {
		case <-t.pongs:
		default:
			return
		}
	}
}

func (t *Transport) finish(err error) {
	t.finishOnce.Do(func() {
		if err == nil {
			err = io.EOF
		}
		t.errMu.Lock()
		t.terminalErr = err
		t.errMu.Unlock()

		t.outgoing.shutdown()
		t.control.incoming.close(err)
		t.media.closeFromTransport(err)
		t.cancel()
		_ = t.conn.Close()
		close(t.done)
	})
}

func (t *Transport) terminalError() error {
	t.errMu.Lock()
	defer t.errMu.Unlock()
	return t.terminalErr
}

func (t *Transport) closedError() error {
	select {
	case <-t.done:
		err := t.terminalError()
		if errors.Is(err, io.EOF) {
			return io.EOF
		}
		return err
	default:
		return nil
	}
}

func normalizeSocketError(err error) error {
	if err == nil || errors.Is(err, net.ErrClosed) || errors.Is(err, websocket.ErrCloseSent) {
		return io.EOF
	}
	var closeError *websocket.CloseError
	if errors.As(err, &closeError) && (closeError.Code == websocket.CloseNormalClosure || closeError.Code == websocket.CloseGoingAway) {
		return io.EOF
	}
	return err
}

func closeResult(err error) error {
	if err == nil || errors.Is(err, io.EOF) || errors.Is(err, net.ErrClosed) || errors.Is(err, websocket.ErrCloseSent) {
		return nil
	}
	return err
}

type semanticControlChannel struct {
	transport *Transport
	incoming  *inbox[rtvbp.Received]
}

func (c *semanticControlChannel) Send(ctx context.Context, data []byte) error {
	return c.transport.enqueue(ctx, websocket.TextMessage, data)
}

func (c *semanticControlChannel) Recv(ctx context.Context) (rtvbp.Received, error) {
	return c.incoming.pop(ctx)
}

type staticMediaChannel struct {
	transport *Transport
	incoming  *inbox[rtvbp.MediaFrame]

	mu         sync.Mutex
	format     rtvbp.MediaFormat
	configured bool
	closed     bool
}

func (m *staticMediaChannel) ID() string {
	return staticAudioID
}

func (m *staticMediaChannel) Format() rtvbp.MediaFormat {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.format
}

func (m *staticMediaChannel) WriteFrame(frame rtvbp.MediaFrame) error {
	m.mu.Lock()
	closed := m.closed
	m.mu.Unlock()
	if closed {
		return io.ErrClosedPipe
	}
	return m.transport.enqueue(context.Background(), websocket.BinaryMessage, frame.Data)
}

func (m *staticMediaChannel) ReadFrame() (rtvbp.MediaFrame, error) {
	return m.incoming.pop(context.Background())
}

func (m *staticMediaChannel) Close() error {
	m.mu.Lock()
	if !m.closed {
		m.closed = true
		m.incoming.close(io.EOF)
	}
	m.mu.Unlock()
	return nil
}

func (m *staticMediaChannel) configure(format rtvbp.MediaFormat) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.closed {
		return io.EOF
	}
	if m.configured && m.format != format {
		return errMediaFormatConflict
	}
	m.configured = true
	m.format = format
	return nil
}

func (m *staticMediaChannel) isClosed() bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.closed
}

func (m *staticMediaChannel) closeFromTransport(err error) {
	m.mu.Lock()
	if !m.closed {
		m.closed = true
		m.incoming.close(err)
	}
	m.mu.Unlock()
}

type outboundMessage struct {
	messageType int
	data        []byte
	close       bool
	written     chan error
}

type outboundQueue struct {
	mu     sync.Mutex
	items  []outboundMessage
	closed bool
	ready  chan struct{}
}

func newOutboundQueue() *outboundQueue {
	return &outboundQueue{ready: make(chan struct{}, 1)}
}

func (q *outboundQueue) push(message outboundMessage) error {
	q.mu.Lock()
	defer q.mu.Unlock()
	if q.closed {
		return io.ErrClosedPipe
	}
	q.items = append(q.items, message)
	signalReady(q.ready)
	return nil
}

func (q *outboundQueue) closeWith(message outboundMessage) bool {
	q.mu.Lock()
	defer q.mu.Unlock()
	if q.closed {
		return false
	}
	q.closed = true
	q.items = append(q.items, message)
	signalReady(q.ready)
	return true
}

func (q *outboundQueue) pop(ctx context.Context) (outboundMessage, error) {
	for {
		q.mu.Lock()
		if len(q.items) != 0 {
			message := q.items[0]
			q.items[0] = outboundMessage{}
			q.items = q.items[1:]
			q.mu.Unlock()
			return message, nil
		}
		closed := q.closed
		ready := q.ready
		q.mu.Unlock()
		if closed {
			return outboundMessage{}, io.EOF
		}
		select {
		case <-ctx.Done():
			return outboundMessage{}, ctx.Err()
		case <-ready:
		}
	}
}

func (q *outboundQueue) shutdown() {
	q.mu.Lock()
	q.closed = true
	q.mu.Unlock()
	signalReady(q.ready)
}

type inbox[T any] struct {
	mu     sync.Mutex
	items  []T
	closed bool
	err    error
	ready  chan struct{}
}

func newInbox[T any]() *inbox[T] {
	return &inbox[T]{ready: make(chan struct{}, 1)}
}

func (q *inbox[T]) push(value T) error {
	q.mu.Lock()
	defer q.mu.Unlock()
	if q.closed {
		return io.ErrClosedPipe
	}
	q.items = append(q.items, value)
	signalReady(q.ready)
	return nil
}

func (q *inbox[T]) pop(ctx context.Context) (T, error) {
	var zero T
	for {
		if err := ctx.Err(); err != nil {
			return zero, err
		}
		q.mu.Lock()
		if len(q.items) != 0 {
			value := q.items[0]
			q.items[0] = zero
			q.items = q.items[1:]
			q.mu.Unlock()
			return value, nil
		}
		if q.closed {
			err := q.err
			q.mu.Unlock()
			if err == nil {
				err = io.EOF
			}
			return zero, err
		}
		ready := q.ready
		q.mu.Unlock()
		select {
		case <-ctx.Done():
			return zero, ctx.Err()
		case <-ready:
		}
	}
}

func (q *inbox[T]) close(err error) {
	q.mu.Lock()
	if !q.closed {
		q.closed = true
		q.err = err
	}
	q.mu.Unlock()
	signalReady(q.ready)
}

func signalReady(ready chan struct{}) {
	select {
	case ready <- struct{}{}:
	default:
	}
}

func cloneBytes(data []byte) []byte {
	if data == nil {
		return nil
	}
	return append([]byte(nil), data...)
}

func keepaliveCloseResult(err error) error {
	if err == nil || errors.Is(err, io.EOF) {
		return nil
	}
	return err
}

func stopTimer(timer *time.Timer) {
	if !timer.Stop() {
		select {
		case <-timer.C:
		default:
		}
	}
}

var _ rtvbp.Transport = (*Transport)(nil)
var _ rtvbp.KeepaliveTransport = (*Transport)(nil)
var _ rtvbp.ControlChannel = (*semanticControlChannel)(nil)
var _ rtvbp.MediaChannel = (*staticMediaChannel)(nil)
