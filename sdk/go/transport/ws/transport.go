package ws

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/gorilla/websocket"
	"github.com/smallnest/ringbuffer"
	"log/slog"
	"net"
	"time"
)

func isControl(frameType int) bool {
	return frameType == websocket.CloseMessage || frameType == websocket.PingMessage || frameType == websocket.PongMessage
}

type wsMessage struct {
	mt      int
	data    []byte
	timeout time.Duration
}

func (m *wsMessage) controlTimeout() time.Duration {
	if m.timeout == 0 {
		return 1 * time.Second
	}
	return m.timeout
}

type WebsocketTransport struct {
	conn          *websocket.Conn
	writeBuf      *ringbuffer.RingBuffer
	readBuf       *ringbuffer.RingBuffer
	cc            *controlChannel
	msgOutCh      chan wsMessage // msgOutCh holds messages to be send out
	msgInCh       chan wsMessage // msgInCh holds messages received from the socket
	done          chan struct{}
	logger        *slog.Logger
	debugMessages bool
}

func (w *WebsocketTransport) Read(p []byte) (n int, err error) {
	return w.readBuf.Read(p)
}

func (w *WebsocketTransport) Write(p []byte) (n int, err error) {
	return w.writeBuf.Write(p)
}

func (w *WebsocketTransport) Closed() <-chan struct{} {
	return w.done
}

func (w *WebsocketTransport) Control() rtvbp.DataChannel {
	return w.cc
}

func (w *WebsocketTransport) Close(ctx context.Context) error {

	w.msgOutCh <- wsMessage{
		mt:   websocket.CloseMessage,
		data: websocket.FormatCloseMessage(websocket.CloseNormalClosure, "Closed"),
	}
	select {
	case <-ctx.Done():
		return fmt.Errorf("close failed: %w", ctx.Err())
	case <-w.done:
		return nil
	}
}

func (w *WebsocketTransport) closeNow() {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	err := w.Close(ctx)
	if err != nil {
		w.logger.Error("close failed", slog.Any("err", err))
	}
}

func (w *WebsocketTransport) process(ctx context.Context) {
	defer func() {
		// close connection
		if err := w.conn.Close(); err != nil {
			w.logger.Error("connection close failed", slog.Any("err", err))
		}

		w.logger.Debug("transport processing done")
	}()

	// Read all messages from connection and store them in msgInCh channel
	go func() {
		defer func() {
			close(w.msgInCh)
			close(w.done)
		}()
		for {
			mt, data, err := w.conn.ReadMessage()

			// on error: return
			if err != nil {
				if websocket.IsCloseError(err, websocket.CloseNormalClosure) {
					w.logger.Debug("connection was closed by other peer", slog.Any("err", err))
				} else {
					w.logger.Error("read failed", slog.Any("err", err))
				}
				return
			}

			w.msgInCh <- wsMessage{mt: mt, data: data}
		}
	}()

	// Read from audio write buffer and send it to the socket
	go func() {
		buf := make([]byte, 1024*1024)
		for {
			n, err := w.writeBuf.Read(buf)
			if err != nil {
				w.logger.Error("read audio from buffer failed", slog.Any("err", err))
				return
			}
			if err := w.conn.WriteMessage(websocket.BinaryMessage, buf[:n]); err != nil {
				w.logger.Error("write text failed", slog.Any("err", err))
				return
			}
		}
	}()

	// Process incoming messages
	// - store control messages in Control Channel (cc)
	// - store binary messages in readBuf
	go func() {
		for {
			select {
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
					w.cc.input <- msg.data
				case websocket.BinaryMessage:
					if _, err := w.readBuf.Write(msg.data); err != nil {
						w.logger.Error("write audio from socket to buffer failed", slog.Any("err", err))
						return
					}
				}
			}
		}
	}()

	go func() {
		select {
		case <-ctx.Done():
			return
		case data, ok := <-w.cc.output:
			if !ok {
				return
			}
			w.msgOutCh <- wsMessage{mt: websocket.TextMessage, data: data}
		}
	}()

	// outgoing messages
	pingTicker := time.NewTicker(5 * time.Second)
	for {
		select {
		case <-w.done:
			return

		case <-ctx.Done():
			w.closeNow()

		case <-pingTicker.C:
			w.msgOutCh <- wsMessage{mt: websocket.PingMessage, data: []byte{}, timeout: 1 * time.Second}

		case msg := <-w.msgOutCh:
			if isControl(msg.mt) {
				w.logger.Debug("send control", slog.Int("mt", msg.mt))
				if err := w.conn.WriteControl(msg.mt, msg.data, time.Now().Add(msg.controlTimeout())); err != nil {
					w.logger.Error("write control failed", slog.Any("err", err))
					return
				}
			} else if msg.mt == websocket.TextMessage {
				w.logger.Debug("send text", slog.String("data", string(msg.data)))
				if w.debugMessages {
					fmt.Printf("MSG(out) -->\n%s\n", prettyJson(msg.data))
				}
				if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
					w.logger.Error("write text failed", slog.Any("err", err))
					return
				}
			}
		}
	}
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
	logger *slog.Logger,
) *WebsocketTransport {

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

	done := make(chan struct{})

	//a1, a2 := audio.NewDuplexBuffers()

	return &WebsocketTransport{
		conn:          conn,
		writeBuf:      ringbuffer.New(1024 * 1024).SetBlocking(true),
		readBuf:       ringbuffer.New(1024 * 1024).SetBlocking(true),
		cc:            newControlChannel(),
		msgOutCh:      make(chan wsMessage, 1),
		msgInCh:       make(chan wsMessage, 1),
		logger:        logger,
		done:          done,
		debugMessages: false,
	}
}
