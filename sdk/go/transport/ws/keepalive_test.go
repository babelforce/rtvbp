package ws

import (
	"context"
	"errors"
	"io"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/gorilla/websocket"
)

func TestKeepaliveHealthyPeerUsesOnlyPingControlFrames(t *testing.T) {
	transport, peer := semanticPair(t)
	var pings atomic.Int32
	peer.SetPingHandler(func(payload string) error {
		pings.Add(1)
		return peer.WriteControl(websocket.PongMessage, []byte(payload), time.Now().Add(time.Second))
	})
	peerDone, peerData := readKeepalivePeer(peer)

	ctx, cancel := context.WithTimeout(context.Background(), 90*time.Millisecond)
	defer cancel()
	err := transport.MonitorKeepalive(ctx, rtvbp.KeepalivePolicy{
		Interval:  5 * time.Millisecond,
		Timeout:   15 * time.Millisecond,
		MaxMisses: 2,
	})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("MonitorKeepalive() error = %v, want context deadline", err)
	}
	if pings.Load() < 3 {
		t.Fatalf("observed %d pings, want at least 3", pings.Load())
	}
	if got := peerData.Load(); got != 0 {
		t.Fatalf("peer observed %d data messages; keepalive must not emit text/catalog ping", got)
	}

	if err := transport.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	awaitPeerStop(t, peerDone)
}

func TestKeepaliveSilentPeerTimesOutAfterExactMaxMisses(t *testing.T) {
	transport, peer := semanticPair(t)
	var pings atomic.Int32
	peer.SetPingHandler(func(string) error {
		pings.Add(1)
		return nil
	})
	peerDone, peerData := readKeepalivePeer(peer)

	err := transport.MonitorKeepalive(context.Background(), rtvbp.KeepalivePolicy{
		Interval:  2 * time.Millisecond,
		Timeout:   8 * time.Millisecond,
		MaxMisses: 3,
	})
	if !errors.Is(err, rtvbp.ErrKeepaliveTimeout) {
		t.Fatalf("MonitorKeepalive() error = %v, want ErrKeepaliveTimeout", err)
	}
	if got := pings.Load(); got != 3 {
		t.Fatalf("observed %d pings, want exactly 3", got)
	}
	if got := peerData.Load(); got != 0 {
		t.Fatalf("peer observed %d data messages; keepalive must not emit text/catalog ping", got)
	}
	if _, err := transport.Control().Recv(context.Background()); !errors.Is(err, rtvbp.ErrKeepaliveTimeout) {
		t.Fatalf("Recv() error = %v, want ErrKeepaliveTimeout", err)
	}
	awaitPeerStop(t, peerDone)
}

func TestKeepaliveMatchingPongResetsMissesAndStalePongDoesNot(t *testing.T) {
	transport, peer := semanticPair(t)
	var pings atomic.Int32
	var previous string
	peer.SetPingHandler(func(payload string) error {
		count := pings.Add(1)
		switch count {
		case 2:
			previous = payload
			return peer.WriteControl(websocket.PongMessage, []byte(payload), time.Now().Add(time.Second))
		case 3:
			return peer.WriteControl(websocket.PongMessage, []byte(previous), time.Now().Add(time.Second))
		default:
			return nil
		}
	})
	peerDone, _ := readKeepalivePeer(peer)

	err := transport.MonitorKeepalive(context.Background(), rtvbp.KeepalivePolicy{
		Interval:  2 * time.Millisecond,
		Timeout:   8 * time.Millisecond,
		MaxMisses: 2,
	})
	if !errors.Is(err, rtvbp.ErrKeepaliveTimeout) {
		t.Fatalf("MonitorKeepalive() error = %v, want ErrKeepaliveTimeout", err)
	}
	// Ping 1 misses, ping 2 matches and resets, ping 3 receives only the stale
	// ping-2 pong, and ping 4 reaches the second consecutive miss.
	if got := pings.Load(); got != 4 {
		t.Fatalf("observed %d pings, want exactly 4", got)
	}
	awaitPeerStop(t, peerDone)
}

func TestKeepaliveStopsCleanlyWithTransport(t *testing.T) {
	transport, peer := semanticPair(t)
	peer.SetPingHandler(func(payload string) error {
		return peer.WriteControl(websocket.PongMessage, []byte(payload), time.Now().Add(time.Second))
	})
	peerDone, _ := readKeepalivePeer(peer)

	monitorDone := make(chan error, 1)
	go func() {
		monitorDone <- transport.MonitorKeepalive(context.Background(), rtvbp.KeepalivePolicy{
			Interval:  2 * time.Millisecond,
			Timeout:   8 * time.Millisecond,
			MaxMisses: 2,
		})
	}()
	time.Sleep(10 * time.Millisecond)
	if err := transport.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	select {
	case err := <-monitorDone:
		if err != nil && !errors.Is(err, io.EOF) {
			t.Fatalf("MonitorKeepalive() error after Close = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("MonitorKeepalive did not stop with transport")
	}
	awaitPeerStop(t, peerDone)
}

func TestConcurrentKeepaliveWaiterHonorsContext(t *testing.T) {
	transport, peer := semanticPair(t)
	var pings atomic.Int32
	peer.SetPingHandler(func(payload string) error {
		pings.Add(1)
		return peer.WriteControl(websocket.PongMessage, []byte(payload), time.Now().Add(time.Second))
	})
	peerDone, _ := readKeepalivePeer(peer)

	policy := rtvbp.KeepalivePolicy{
		Interval:  2 * time.Millisecond,
		Timeout:   20 * time.Millisecond,
		MaxMisses: 2,
	}
	firstDone := make(chan error, 1)
	go func() {
		firstDone <- transport.MonitorKeepalive(context.Background(), policy)
	}()
	deadline := time.Now().Add(time.Second)
	for pings.Load() == 0 && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if pings.Load() == 0 {
		t.Fatal("first keepalive monitor did not start")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	if err := transport.MonitorKeepalive(ctx, policy); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("concurrent MonitorKeepalive() error = %v, want context deadline", err)
	}

	if err := transport.Close(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := <-firstDone; err != nil && !errors.Is(err, io.EOF) {
		t.Fatalf("first MonitorKeepalive() error after Close = %v", err)
	}
	awaitPeerStop(t, peerDone)
}

func readKeepalivePeer(peer *websocket.Conn) (<-chan struct{}, *atomic.Int32) {
	done := make(chan struct{})
	dataMessages := &atomic.Int32{}
	go func() {
		defer close(done)
		for {
			messageType, _, err := peer.ReadMessage()
			if err != nil {
				return
			}
			if messageType == websocket.TextMessage || messageType == websocket.BinaryMessage {
				dataMessages.Add(1)
			}
		}
	}()
	return done, dataMessages
}

func awaitPeerStop(t *testing.T, done <-chan struct{}) {
	t.Helper()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("peer reader did not stop")
	}
}
