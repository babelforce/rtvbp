package webrtcws

import (
	"bytes"
	"context"
	"encoding/binary"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/gorilla/websocket"
)

func TestPionTransportCarriesControlAndTimedDuplexAudio(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	serverTransport := make(chan *Transport, 1)
	serverErr := make(chan error, 1)
	upgrader := websocket.Upgrader{Subprotocols: []string{Subprotocol}}
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		conn, err := upgrader.Upgrade(writer, request, nil)
		if err != nil {
			serverErr <- err
			return
		}
		base, err := ws.NewTransport(context.Background(), conn, nil)
		if err != nil {
			serverErr <- err
			return
		}
		transport, err := newTransport(base, Config{AudioFormat: testAudioFormat()})
		if err == nil {
			err = negotiateAnswer(ctx, transport.Control(), v1classic.Envelope{}, transport.peer)
		}
		if err != nil {
			_ = base.Close(context.Background())
			serverErr <- err
			return
		}
		serverTransport <- transport
	}))
	defer server.Close()

	base, err := ws.DialDetached(ctx, ws.ClientConfig{
		Dial:         ws.DialConfig{URL: "ws" + strings.TrimPrefix(server.URL, "http")},
		Subprotocols: []string{Subprotocol},
	})
	if err != nil {
		t.Fatalf("dial WebSocket control: %v", err)
	}
	client, err := newTransport(base, Config{})
	if err != nil {
		t.Fatalf("create client transport: %v", err)
	}
	t.Cleanup(func() { _ = client.Close(context.Background()) })
	if err := negotiateOffer(ctx, client.Control(), v1classic.Envelope{}, client.peer); err != nil {
		t.Fatalf("negotiate offer: %v", err)
	}
	var accepted *Transport
	select {
	case accepted = <-serverTransport:
	case err := <-serverErr:
		t.Fatalf("answer transport: %v", err)
	case <-ctx.Done():
		t.Fatal(ctx.Err())
	}
	t.Cleanup(func() { _ = accepted.Close(context.Background()) })

	assertControlAfterSignaling(t, ctx, client, accepted)
	if _, err := client.OpenMedia(ctx, "video", testAudioFormat()); err != rtvbp.ErrMediaUnsupported {
		t.Fatalf("unsupported media error = %v, want %v", err, rtvbp.ErrMediaUnsupported)
	}
	clientMedia, serverMedia := openMediaPair(t, ctx, client, accepted)
	assertDuplexAudio(t, clientMedia, serverMedia)
	if !strings.Contains(client.peer.RemoteDescription().SDP, "PCMU/8000") {
		t.Fatalf("client remote SDP did not select PCMU:\n%s", client.peer.RemoteDescription().SDP)
	}
	if err := accepted.Close(context.Background()); err != nil {
		t.Fatalf("close accepted transport: %v", err)
	}
	if err := accepted.Close(context.Background()); err != nil {
		t.Fatalf("close accepted transport twice: %v", err)
	}
	if err := client.Close(context.Background()); err != nil {
		t.Fatalf("close client transport: %v", err)
	}
	if err := client.Close(context.Background()); err != nil {
		t.Fatalf("close client transport twice: %v", err)
	}
}

func assertControlAfterSignaling(t *testing.T, ctx context.Context, client, server *Transport) {
	t.Helper()
	envelope := v1classic.Envelope{}
	requestBytes, err := envelope.Encode(rtvbp.ControlFrame{
		Kind:    rtvbp.KindRequest,
		ID:      "control-after-sdp",
		Method:  babelforcev1.MethodPing,
		Payload: []byte(`{"t0":1}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := client.Control().Send(ctx, requestBytes); err != nil {
		t.Fatal(err)
	}
	received, err := server.Control().Recv(ctx)
	if err != nil {
		t.Fatal(err)
	}
	frame, err := envelope.Decode(received.Data)
	if err != nil {
		t.Fatal(err)
	}
	if frame.Kind != rtvbp.KindRequest || frame.Method != babelforcev1.MethodPing || frame.ID != "control-after-sdp" {
		t.Fatalf("post-signaling control frame = %#v", frame)
	}
}

func openMediaPair(t *testing.T, ctx context.Context, client, server *Transport) (rtvbp.MediaChannel, rtvbp.MediaChannel) {
	t.Helper()
	type result struct {
		channel rtvbp.MediaChannel
		err     error
	}
	clientResult := make(chan result, 1)
	serverResult := make(chan result, 1)
	go func() {
		channel, err := client.OpenMedia(ctx, audioID, testAudioFormat())
		clientResult <- result{channel: channel, err: err}
	}()
	go func() {
		channel, err := server.AcceptMedia(ctx)
		serverResult <- result{channel: channel, err: err}
	}()
	openedClient := <-clientResult
	if openedClient.err != nil {
		t.Fatalf("open client media: %v", openedClient.err)
	}
	acceptedServer := <-serverResult
	if acceptedServer.err != nil {
		t.Fatalf("accept server media: %v", acceptedServer.err)
	}
	if _, err := client.OpenMedia(ctx, audioID, testAudioFormat()); err != errMediaClaimed {
		t.Fatalf("duplicate OpenMedia error = %v, want %v", err, errMediaClaimed)
	}
	return openedClient.channel, acceptedServer.channel
}

func assertDuplexAudio(t *testing.T, client, server rtvbp.MediaChannel) {
	t.Helper()
	clientFrames := [][]byte{pcmFrame(1000), pcmFrame(-1000)}
	for _, data := range clientFrames {
		if err := client.WriteFrame(rtvbp.MediaFrame{Data: data}); err != nil {
			t.Fatalf("write client audio: %v", err)
		}
	}
	for index, sent := range clientFrames {
		received, err := server.ReadFrame()
		if err != nil {
			t.Fatalf("read server audio %d: %v", index, err)
		}
		if !received.Timed || received.PTS != time.Duration(index)*pcmuPTime {
			t.Fatalf("server frame %d timing = timed:%v pts:%s", index, received.Timed, received.PTS)
		}
		if want := decodePCMU(encodePCMU(sent)); !bytes.Equal(received.Data, want) {
			t.Fatalf("server frame %d audio mismatch", index)
		}
	}

	fromServer := pcmFrame(3000)
	if err := server.WriteFrame(rtvbp.MediaFrame{Data: fromServer}); err != nil {
		t.Fatalf("write server audio: %v", err)
	}
	received, err := client.ReadFrame()
	if err != nil {
		t.Fatalf("read client audio: %v", err)
	}
	if !received.Timed || !bytes.Equal(received.Data, decodePCMU(encodePCMU(fromServer))) {
		t.Fatalf("client audio = timed:%v bytes:%d", received.Timed, len(received.Data))
	}
}

func pcmFrame(sample int16) []byte {
	frame := make([]byte, 320)
	for offset := 0; offset < len(frame); offset += 2 {
		binary.LittleEndian.PutUint16(frame[offset:], uint16(sample))
	}
	return frame
}

func TestAddToServerRetainsPlainWebSocketBinding(t *testing.T) {
	configured := AddToServer(ws.ServerConfig{}, Config{AudioFormat: testAudioFormat()})
	if len(configured.Subprotocols) != 2 || configured.Subprotocols[0] != ws.DefaultSubprotocol || configured.Subprotocols[1] != Subprotocol {
		t.Fatalf("subprotocols = %v, want [%s %s]", configured.Subprotocols, Subprotocol, ws.DefaultSubprotocol)
	}
	configured = AddToServer(ws.ServerConfig{Subprotocols: []string{ws.DefaultSubprotocol}}, Config{AudioFormat: testAudioFormat()})
	if len(configured.Subprotocols) != 2 || configured.Subprotocols[0] != ws.DefaultSubprotocol {
		t.Fatalf("explicit plain binding not retained: %v", configured.Subprotocols)
	}
}

func TestPlainWebSocketClientStillWorksOnCombinedServer(t *testing.T) {
	server := ws.NewServer(AddToServer(ws.ServerConfig{
		Addr: "127.0.0.1:0",
	}, Config{AudioFormat: testAudioFormat()}), rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	if err := server.Listen(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = server.Shutdown(context.Background()) })
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	plain, err := ws.Dial(ctx, server.GetClientConfig())
	if err != nil {
		t.Fatalf("dial plain WebSocket binding: %v", err)
	}
	if plain.WireSubprotocol() != ws.DefaultSubprotocol {
		t.Fatalf("selected subprotocol = %q, want %q", plain.WireSubprotocol(), ws.DefaultSubprotocol)
	}
	if err := plain.Close(context.Background()); err != nil {
		t.Fatalf("close plain WebSocket binding: %v", err)
	}
}
