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
	connMu        sync.Mutex
	conn          *websocket.Conn
	audio         io.ReadWriter
	config        *TransportConfig
	cc            *controlChannel
	msgOutCh      chan wsMessage // msgOutCh holds messages to be send out
	msgInCh       chan wsMessage // msgInCh holds messages received from the socket
	wsClosedCh    chan struct{}
	closeCh       chan struct{}
	logger        *slog.Logger
	debugMessages bool
	closeOnce     sync.Once
}

func (w *WebsocketTransport) Closed() <-chan struct{} {
	return w.wsClosedCh
}

func (w *WebsocketTransport) Control() rtvbp.DataChannel {
	return w.cc
}

// Close closes the websocket transport
// will send all currently buffered outgoing messages
// and wait for the connection to be closed
func (w *WebsocketTransport) Close(ctx context.Context) error {

	select {
	case <-w.wsClosedCh:
		return nil
	default:
	}

	w.closeOnce.Do(func() {
		close(w.closeCh)
	})

	w.drainOut()

	w.msgOutCh <- wsMessage{
		mt:   websocket.CloseMessage,
		data: websocket.FormatCloseMessage(websocket.CloseNormalClosure, "Closed"),
	}

	select {
	case <-ctx.Done():
		return fmt.Errorf("close failed: %w", ctx.Err())
	case <-w.wsClosedCh:
		return nil
	}
}

func (w *WebsocketTransport) drainOut() {
	w.logger.Debug("drain outgoing messages", slog.Int("len", len(w.msgOutCh)))
	for {
		select {
		case msg := <-w.msgOutCh:
			err := w.sendMessage(msg)
			if err != nil {
				w.logger.Error("drainOut: failed to send message", slog.Any("err", err))
			}
		default:
			w.logger.Debug("drained outgoing messages")
			return
		}
	}
}

func (w *WebsocketTransport) closeWithTimeout(timeout time.Duration) error {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	return w.Close(ctx)
}

func (w *WebsocketTransport) process(ctx context.Context) {
	// cleanup
	defer func() {
		_ = w.conn.Close()
	}()

	// Read all messages from connection and store them in msgInCh channel
	go func() {
		for {
			mt, data, err := w.conn.ReadMessage()

			// on error: return
			if err != nil {

				var e *websocket.CloseError
				if errors.As(err, &e) {
					close(w.wsClosedCh)
					return
				}

				w.logger.Error("read failed", slog.Any("err", err))

				return
			}

			w.msgInCh <- wsMessage{mt: mt, data: data}
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
				w.logger.Error("read audio from buffer failed", slog.Any("err", err))
				return
			}

			// TODO: buffer reuse for more efficient memory allocations
			data := make([]byte, n)
			copy(data, buf[:n])

			select {
			case <-w.closeCh:
				return
			case w.msgOutCh <- wsMessage{mt: websocket.BinaryMessage, data: data}:
			}

		}
	}()

	// Process incoming messages
	// - store control messages in Control Channel (cc)
	// - store binary messages in readBuf
	go func() {
		for {
			select {
			case <-w.wsClosedCh:
				return
			case <-ctx.Done():
				return
			case msg, ok := <-w.msgInCh:
				if !ok {
					return
				}
				switch msg.mt {
				case websocket.TextMessage:
					if w.debugMessages {
						fmt.Printf("MSG(in) <--\n%s\n", prettyJson(msg.data))
					}
					w.cc.input <- rtvbp.DataPackage{Data: msg.data, ReceivedAt: time.Now().UnixMilli()}
				case websocket.BinaryMessage:
					if _, err := w.audio.Write(msg.data); err != nil {
						w.logger.Error("write audio from socket to buffer failed", slog.Any("err", err))
						return
					}
				}
			}
		}
	}()

	// outgoing messages
	go func() {
		pingTicker := time.NewTicker(5 * time.Second)
		for {
			select {

			case <-w.wsClosedCh:
				return

			case <-pingTicker.C:
				w.msgOutCh <- wsMessage{mt: websocket.PingMessage, data: []byte{}, timeout: 1 * time.Second}

			case msg := <-w.msgOutCh:
				err := w.sendMessage(msg)
				if err != nil {
					w.logger.Error("failed to send message", slog.Any("err", err))
					return
				}
			}
		}
	}()

	for {
		select {
		case <-ctx.Done():
			_ = w.closeWithTimeout(5 * time.Second)
			return

		case <-w.wsClosedCh:
			return
		}
	}
}

func (w *WebsocketTransport) sendMessage(msg wsMessage) error {
	w.connMu.Lock()
	defer w.connMu.Unlock()
	if isControl(msg.mt) {
		w.logger.Debug("send control", slog.Int("mt", msg.mt))
		if err := w.conn.WriteControl(msg.mt, msg.data, time.Now().Add(msg.controlTimeout())); err != nil {
			return fmt.Errorf("write control failed: %w", err)
		}
	} else if msg.mt == websocket.TextMessage {
		w.logger.Debug("send text", slog.String("data", string(msg.data)))
		if w.debugMessages {
			fmt.Printf("MSG(out) -->\n%s\n", prettyJson(msg.data))
		}
		if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
			return fmt.Errorf("write text failed: %w", err)
		}
	} else if msg.mt == websocket.BinaryMessage {
		if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
			return fmt.Errorf("write binary failed: %w", err)
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

	logger = logger.With()

	logger.Debug(
		"new transport",
	)

	conn.SetPingHandler(func(message string) error {
		logger.Debug("received ping")
		err := conn.WriteControl(websocket.PongMessage, []byte(message), time.Now().Add(1*time.Second))
		if err == websocket.ErrCloseSent {
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
		conn:          conn,
		audio:         audio,
		config:        config,
		cc:            newControlChannel(msgOutCh),
		msgOutCh:      msgOutCh,
		msgInCh:       make(chan wsMessage, 16),
		closeCh:       make(chan struct{}),
		logger:        logger,
		wsClosedCh:    wsClosedCh,
		debugMessages: false,
	}
}
