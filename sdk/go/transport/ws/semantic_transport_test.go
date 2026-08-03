package ws

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/gorilla/websocket"
)

func TestSemanticControlRoundTripAndCancellation(t *testing.T) {
	transport, peer := semanticPair(t)

	want := []byte(`{"version":"1","id":"request-1","method":"ping"}`)
	if err := transport.Control().Send(context.Background(), want); err != nil {
		t.Fatalf("Send() error = %v", err)
	}
	messageType, data, err := peer.ReadMessage()
	if err != nil {
		t.Fatalf("peer ReadMessage() error = %v", err)
	}
	if messageType != websocket.TextMessage || string(data) != string(want) {
		t.Fatalf("peer message = (%d, %q), want text %q", messageType, data, want)
	}

	if err := peer.WriteMessage(websocket.TextMessage, []byte("reply")); err != nil {
		t.Fatalf("peer WriteMessage() error = %v", err)
	}
	received, err := transport.Control().Recv(context.Background())
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	if string(received.Data) != "reply" || received.ReceivedAt.IsZero() {
		t.Fatalf("Recv() = %#v", received)
	}

	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := transport.Control().Recv(canceled); !errors.Is(err, context.Canceled) {
		t.Fatalf("Recv(canceled) error = %v, want context.Canceled", err)
	}
}

func TestSemanticStaticAudioRoundTrip(t *testing.T) {
	transport, peer := semanticPair(t)
	format := rtvbp.MediaFormat{
		Encoding:   "L16",
		SampleRate: 16000,
		Channels:   1,
		PTime:      20 * time.Millisecond,
	}
	media, err := transport.OpenMedia(context.Background(), "audio", format)
	if err != nil {
		t.Fatalf("OpenMedia() error = %v", err)
	}
	if media.ID() != "audio" || media.Format() != format {
		t.Fatalf("media = (%q, %#v)", media.ID(), media.Format())
	}
	if _, err := transport.OpenMedia(context.Background(), "video", format); !errors.Is(err, rtvbp.ErrMediaUnsupported) {
		t.Fatalf("OpenMedia(video) error = %v, want ErrMediaUnsupported", err)
	}

	data := []byte{1, 2, 3, 4}
	if err := media.WriteFrame(rtvbp.MediaFrame{Data: data, PTS: time.Second, Timed: true}); err != nil {
		t.Fatalf("WriteFrame() error = %v", err)
	}
	data[0] = 9
	messageType, wire, err := peer.ReadMessage()
	if err != nil {
		t.Fatalf("peer ReadMessage() error = %v", err)
	}
	if messageType != websocket.BinaryMessage || string(wire) != string([]byte{1, 2, 3, 4}) {
		t.Fatalf("peer media = (%d, %v)", messageType, wire)
	}

	if err := peer.WriteMessage(websocket.BinaryMessage, []byte{5, 6}); err != nil {
		t.Fatalf("peer WriteMessage() error = %v", err)
	}
	received, err := media.ReadFrame()
	if err != nil {
		t.Fatalf("ReadFrame() error = %v", err)
	}
	if string(received.Data) != string([]byte{5, 6}) || received.Timed || received.PTS != 0 {
		t.Fatalf("ReadFrame() = %#v", received)
	}
}

func TestSemanticCloseFlushesEveryAdmittedControlFrame(t *testing.T) {
	transport, peer := semanticPair(t)
	const count = 128
	for index := 0; index < count; index++ {
		if err := transport.Control().Send(context.Background(), []byte(fmt.Sprintf("frame-%03d", index))); err != nil {
			t.Fatalf("Send(%d) error = %v", index, err)
		}
	}

	closeCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := transport.Close(closeCtx); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	for index := 0; index < count; index++ {
		messageType, data, err := peer.ReadMessage()
		if err != nil {
			t.Fatalf("peer ReadMessage(%d) error = %v", index, err)
		}
		want := fmt.Sprintf("frame-%03d", index)
		if messageType != websocket.TextMessage || string(data) != want {
			t.Fatalf("peer message %d = (%d, %q), want text %q", index, messageType, data, want)
		}
	}
	if _, _, err := peer.ReadMessage(); err == nil {
		t.Fatal("peer did not observe websocket close")
	}
	if err := transport.Control().Send(context.Background(), []byte("late")); !errors.Is(err, io.ErrClosedPipe) {
		t.Fatalf("Send after Close error = %v, want io.ErrClosedPipe", err)
	}
}

func TestUnreadBinaryDoesNotBlockControl(t *testing.T) {
	transport, peer := semanticPair(t)
	for index := 0; index < 256; index++ {
		if err := peer.WriteMessage(websocket.BinaryMessage, []byte{byte(index)}); err != nil {
			t.Fatalf("peer binary WriteMessage(%d) error = %v", index, err)
		}
	}
	if err := peer.WriteMessage(websocket.TextMessage, []byte("control")); err != nil {
		t.Fatalf("peer text WriteMessage() error = %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	received, err := transport.Control().Recv(ctx)
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	if string(received.Data) != "control" {
		t.Fatalf("Recv() data = %q", received.Data)
	}
}

func semanticPair(t *testing.T) (*Transport, *websocket.Conn) {
	t.Helper()
	accepted := make(chan *websocket.Conn, 1)
	upgrader := websocket.Upgrader{}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, request *http.Request) {
		conn, err := upgrader.Upgrade(w, request, nil)
		if err != nil {
			return
		}
		accepted <- conn
	}))
	t.Cleanup(server.Close)

	url := "ws" + strings.TrimPrefix(server.URL, "http")
	client, _, err := websocket.DefaultDialer.Dial(url, nil)
	if err != nil {
		t.Fatalf("Dial() error = %v", err)
	}
	peer := <-accepted
	ctx, cancel := context.WithCancel(context.Background())
	transport := NewTransport(ctx, client, nil)
	t.Cleanup(func() {
		closeCtx, closeCancel := context.WithTimeout(context.Background(), time.Second)
		defer closeCancel()
		_ = transport.Close(closeCtx)
		_ = peer.Close()
		cancel()
	})
	return transport, peer
}
