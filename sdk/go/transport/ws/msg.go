package ws

import (
	"time"

	"github.com/gorilla/websocket"
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
