package ws

import (
	"babelforce.go/ivr/rtvbp/rtvbp-go"
	"babelforce.go/ivr/rtvbp/rtvbp-go/audio"
	"context"
	"fmt"
	"github.com/gorilla/websocket"
	"log/slog"
	"net"
	"time"
)

func isControl(frameType int) bool {
	return frameType == websocket.CloseMessage || frameType == websocket.PingMessage || frameType == websocket.PongMessage
}

func isData(frameType int) bool {
	return frameType == websocket.TextMessage || frameType == websocket.BinaryMessage
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
	conn         *websocket.Conn
	audioSession *audio.DuplexBuffer
	audioLocal   *audio.DuplexBuffer
	cc           *controlChannel
	msgOut       chan wsMessage // msgOut holds messages to be send out
	chTextRcv    chan []byte
	done         chan struct{}
	logger       *slog.Logger
}

func (w *WebsocketTransport) Read(p []byte) (n int, err error) {
	return w.audioSession.Read(p)
}

func (w *WebsocketTransport) Write(p []byte) (n int, err error) {
	return w.audioSession.Write(p)
}

func (w *WebsocketTransport) Closed() <-chan struct{} {
	return w.done
}

func (w *WebsocketTransport) Control() rtvbp.DataChannel {
	return w.cc
}

func (w *WebsocketTransport) Close(ctx context.Context) error {

	w.msgOut <- wsMessage{
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

func (w *WebsocketTransport) processConnection(ctx context.Context) {
	defer func() {
		// close connection
		if err := w.conn.Close(); err != nil {
			w.logger.Error("connection close failed", slog.Any("err", err))
		}

		w.logger.Debug("transport processing done")
	}()

	// read messages from connection and write them to incoming channel
	// Note: this does not contain any control messages!
	go func() {
		defer close(w.done)
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

			switch mt {
			case websocket.TextMessage:
				w.cc.input <- data
			case websocket.BinaryMessage:
				//w.logger.Debug("rcv binary", slog.Int("len", len(data)))
				if _, err := w.audioLocal.Write(data); err != nil {
					w.logger.Error("write audio from socket to buffer failed", slog.Any("err", err))
					return
				}
			}
		}
	}()

	go func() {
		buf := make([]byte, 8_000)
		for {
			n, err := w.audioLocal.Read(buf)
			if err != nil {
				w.logger.Error("read audio from buffer failed", slog.Any("err", err))
				return
			}
			w.msgOut <- wsMessage{mt: websocket.BinaryMessage, data: buf[:n]}
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
			w.msgOut <- wsMessage{mt: websocket.PingMessage, data: []byte{}, timeout: 1 * time.Second}

		case data, ok := <-w.cc.output:
			if !ok {
				continue
			}
			w.msgOut <- wsMessage{mt: websocket.TextMessage, data: data}

		case msg := <-w.msgOut:
			if isControl(msg.mt) {
				w.logger.Debug("send control", slog.Int("mt", msg.mt))
				if err := w.conn.WriteControl(msg.mt, msg.data, time.Now().Add(msg.controlTimeout())); err != nil {
					w.logger.Error("write control failed", slog.Any("err", err))
					return
				}
			} else if isData(msg.mt) {
				if msg.mt == websocket.BinaryMessage {
					//w.logger.Debug("send binary", slog.Int("len", len(msg.data)))
				} else {
					w.logger.Debug("send text", slog.String("data", string(msg.data)))
				}
				if err := w.conn.WriteMessage(msg.mt, msg.data); err != nil {
					w.logger.Error("write text failed", slog.Any("err", err))
					return
				}
			}
		}
	}
}

var _ rtvbp.Transport = &WebsocketTransport{}

func newTransport(
	conn *websocket.Conn,
	logger *slog.Logger,
) *WebsocketTransport {
	var (
		bufSize = 1
		msgOut  = make(chan wsMessage, bufSize)
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

	done := make(chan struct{})

	a1, a2 := audio.NewDuplexBuffers()

	return &WebsocketTransport{
		conn:         conn,
		audioSession: a1,
		audioLocal:   a2,
		cc:           newControlChannel(),
		msgOut:       msgOut,
		logger:       logger,
		done:         done,
	}
}
