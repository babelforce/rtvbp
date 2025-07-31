package ws

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
)

type WebsocketTransport struct {
	connMu     sync.Mutex
	conn       *websocket.Conn
	audio      io.ReadWriter
	config     *TransportConfig
	cc         *controlChannel
	msgOutChan chan wsMessage // msgOutChan holds messages to be send out
	msgInChan  chan wsMessage // msgInChan holds messages received from the socket
	doneChan   chan struct{}  // doneChan gets closed when done
	closeChan  chan struct{}  // closeChan is closed to trigger teardown
	closeOnce  sync.Once
	logger     *slog.Logger
}

func (w *WebsocketTransport) Write(data []byte) error {
	return w.cc.writeOut(data)
}

func (w *WebsocketTransport) ReadChan() <-chan rtvbp.DataPackage {
	return w.cc.readChan()
}

func (w *WebsocketTransport) doClose() {
	w.closeOnce.Do(func() {
		close(w.closeChan)
	})
}

// Close closes the websocket transport
// will send all currently buffered outgoing messages
// and wait for the connection to be closed
func (w *WebsocketTransport) Close(ctx context.Context) error {

	// check if we are already done
	select {
	case <-w.doneChan:
		return nil
	default:
	}

	// teardown
	w.doClose()

	// graceful sending out messages
	w.drainOutgoingMessages()
	w.msgOutChan <- wsMessage{
		mt:   websocket.CloseMessage,
		data: websocket.FormatCloseMessage(websocket.CloseNormalClosure, "Closed"),
	}

	// wait until done
	select {
	case <-ctx.Done():
		return fmt.Errorf("close failed: %w", ctx.Err())
	case <-w.doneChan:
		return nil
	}
}

func (w *WebsocketTransport) drainOutgoingMessages() {
	w.logger.Debug("drain outgoing messages", slog.Int("len", len(w.msgOutChan)))
	for {
		select {
		case msg, ok := <-w.msgOutChan:
			if !ok {
				return
			}

			err := w.sendMessage(msg)
			if errors.Is(err, websocket.ErrCloseSent) {
				return
			}
			if errors.Is(err, net.ErrClosed) {
				return
			}
			if err != nil {
				w.logger.Error("drainOutgoingMessages: failed to send message", slog.Any("err", err))
			}
		default:
			w.logger.Debug("drained outgoing messages")
			return
		}
	}
}

func (w *WebsocketTransport) closeIn(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	return w.Close(ctx)
}

func (w *WebsocketTransport) process(ctx context.Context) {
	// cleanup
	defer func() {
		_ = w.conn.Close() // close connection
		w.cc.close()       // close channels
		close(w.doneChan)  // signal transport ended
	}()

	// Read all messages from connection and store them in msgInCh channel
	go func() {
		defer w.doClose()
		for {
			mt, data, err := w.conn.ReadMessage()

			if err != nil {
				var e *websocket.CloseError
				if errors.As(err, &e) {
					return
				}
				if errors.Is(err, net.ErrClosed) {
					return
				}
				w.logger.Error("read failed", slog.Any("err", err))
				return
			}

			w.msgInChan <- wsMessage{mt: mt, data: data}
		}
	}()

	// Read audio data and send to socket
	go func() {
		buf := make([]byte, 320)
		if w.audio == nil {
			return
		}
		for {
			n, err := w.audio.Read(buf)
			if err != nil {
				if errors.Is(err, io.EOF) {
					return
				}
				w.logger.Error("read audio from buffer failed", slog.Any("err", err))
				return
			}

			// TODO: buffer reuse for more efficient memory allocations
			data := make([]byte, n)
			copy(data, buf[:n])

			select {
			case <-w.doneChan:
				return
			case <-w.closeChan:
				return
			case w.msgOutChan <- wsMessage{mt: websocket.BinaryMessage, data: data}:
			}
		}
	}()

	// Process incoming messages
	// - store control messages in Control Channel (cc)
	// - store binary messages in readBuf
	go func() {
		for {
			select {
			case <-w.doneChan:
				return
			case <-ctx.Done():
				return
			case msg, ok := <-w.msgInChan:
				if !ok {
					return
				}
				switch msg.mt {
				case websocket.TextMessage:
					if w.config.Debug {
						fmt.Printf("MSG(in) <--\n%s\n", prettyJson(msg.data))
					}
					if err := w.cc.writeIn(rtvbp.DataPackage{Data: msg.data, ReceivedAt: time.Now().UnixMilli()}); err != nil {
						w.logger.Error("writeIn failed", slog.Any("err", err))
						return
					}
				case websocket.BinaryMessage:
					if _, err := w.audio.Write(msg.data); err != nil {
						w.logger.Error("writeOut audio from socket to buffer failed", slog.Any("err", err))
						return
					}
				}
			}
		}
	}()

	// outgoing messages
	go func() {
		pingTicker := time.NewTicker(5 * time.Second)
		defer pingTicker.Stop()

		for {
			select {
			case <-ctx.Done():
				_ = w.closeIn(5 * time.Second)
				return

			case <-w.doneChan:
				return

			case <-pingTicker.C:
				w.msgOutChan <- wsMessage{mt: websocket.PingMessage, data: []byte{}, timeout: 1 * time.Second}

			case msg, ok := <-w.msgOutChan:
				if !ok {
					return
				}
				err := w.sendMessage(msg)
				if err != nil {
					if isErrOk(err) {
						return
					}
					w.logger.Error("failed to send message", slog.Any("err", err))
					return
				}
			}
		}
	}()

	select {
	case <-ctx.Done():
	case <-w.doneChan:
	case <-w.closeChan:
	}
}

func isErrOk(err error) bool {
	if err == nil {
		return true
	}
	if errors.Is(err, websocket.ErrCloseSent) {
		return true
	}
	if errors.Is(err, net.ErrClosed) {
		return true
	}
	return false
}

func (w *WebsocketTransport) sendMessage(msg wsMessage) error {
	w.connMu.Lock()
	defer w.connMu.Unlock()

	_ = w.conn.SetWriteDeadline(time.Now().Add(5 * time.Second))

	if isControl(msg.mt) {
		w.logger.Debug("send control", slog.Int("mt", msg.mt))
		if err := w.conn.WriteControl(msg.mt, msg.data, time.Now().Add(msg.controlTimeout())); err != nil {
			return fmt.Errorf("control: %w", err)
		}
	} else if msg.mt == websocket.TextMessage {
		w.logger.Debug("send text", slog.String("data", string(msg.data)))
		if w.config.Debug {
			fmt.Printf("MSG(out) -->\n%s\n", prettyJson(msg.data))
		}
		if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
			return fmt.Errorf("text: %w", err)
		}
	} else if msg.mt == websocket.BinaryMessage {
		if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
			return fmt.Errorf("binary: %w", err)
		} else {
			//w.logger.Debug("sent", slog.Int("len", len(msg.data)))
		}
	}

	return nil
}

func prettyJson(data []byte) string {
	var d map[string]any
	if err := json.Unmarshal(data, &d); err != nil {
		return string(data)
	}
	x, _ := json.MarshalIndent(d, "", "  ")
	return string(x)
}

var _ rtvbp.Transport = &WebsocketTransport{}

func newTransport(
	conn *websocket.Conn,
	audio io.ReadWriter,
	config *TransportConfig,
) *WebsocketTransport {
	// setup logger
	logger := config.Logger
	if logger == nil {
		logger = slog.Default()
	}

	logger.Debug(
		"new transport",
	)

	conn.SetPingHandler(func(message string) error {
		logger.Debug("received ping")
		err := conn.WriteControl(websocket.PongMessage, []byte(message), time.Now().Add(1*time.Second))
		if errors.Is(err, websocket.ErrCloseSent) {
			return nil
		} else if e, ok := err.(net.Error); ok && e.Temporary() {
			return nil
		}
		return err
	})

	conn.SetPongHandler(func(string) error {
		logger.Debug("received pong")
		return nil
	})

	wsClosedCh := make(chan struct{})
	msgOutCh := make(chan wsMessage, 16)

	return &WebsocketTransport{
		conn:       conn,
		audio:      audio,
		config:     config,
		cc:         newControlChannel(msgOutCh),
		msgOutChan: msgOutCh,
		msgInChan:  make(chan wsMessage, 16),
		closeChan:  make(chan struct{}),
		logger:     logger,
		doneChan:   wsClosedCh,
	}
}
