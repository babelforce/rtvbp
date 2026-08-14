package main

import (
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/webrtcws"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
)

func main() {
	if len(os.Args) < 2 {
		panic("usage: typescript-interop server|client [url]")
	}
	var err error
	switch os.Args[1] {
	case "server":
		err = serve()
	case "browser-server":
		if len(os.Args) != 3 {
			panic("browser-server requires websocket or webrtc")
		}
		err = serveBrowser(os.Args[2])
	case "client":
		if len(os.Args) != 3 {
			panic("client requires a WebSocket URL")
		}
		err = client(os.Args[2])
	default:
		panic("unknown mode")
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func serveBrowser(binding string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	ready := make(chan rtvbp.SHC, 1)
	bargeReady := make(chan struct{}, 1)
	handler := applicationHandlerWithPing(func(ctx context.Context, shc rtvbp.SHC) error {
		if err := shc.OpenAudio(ctx, audioFormat()); err != nil {
			return err
		}
		ready <- shc
		return nil
	}, browserPingHandler(bargeReady))
	config := ws.ServerConfig{
		Addr:        "127.0.0.1:0",
		AudioFormat: audioFormat(),
	}
	switch binding {
	case "websocket":
	case "webrtc":
		config = webrtcws.AddToServer(config, webrtcws.Config{AudioFormat: audioFormat()})
	default:
		return fmt.Errorf("unknown browser binding %q", binding)
	}
	server := ws.NewServer(config, handler)
	if err := server.Listen(); err != nil {
		return err
	}
	defer server.Shutdown(context.Background())
	fmt.Println(server.URL())

	var shc rtvbp.SHC
	select {
	case shc = <-ready:
	case <-ctx.Done():
		return ctx.Err()
	}
	received := make(chan error, 1)
	go func() {
		var err error
		for range 100 {
			frame := make([]byte, 320)
			if _, err = io.ReadFull(shc.AudioStream(), frame); err != nil {
				break
			}
			if !bytes.Equal(frame, make([]byte, len(frame))) {
				received <- nil
				return
			}
		}
		if err == nil {
			err = errors.New("received only silent browser microphone audio")
		}
		received <- err
	}()
	sequence := 0
	for range 8 {
		if _, err := shc.AudioStream().Write(toneFrame(sequence)); err != nil {
			return fmt.Errorf("write paced browser audio: %w", err)
		}
		sequence++
		time.Sleep(20 * time.Millisecond)
	}
	for range 32 {
		if _, err := shc.AudioStream().Write(toneFrame(sequence)); err != nil {
			return fmt.Errorf("write buffered browser audio: %w", err)
		}
		sequence++
	}
	if binding == "websocket" {
		select {
		case <-bargeReady:
		case <-ctx.Done():
			return fmt.Errorf("wait for buffered browser playback: %w", ctx.Err())
		}
	}
	if _, err := babelforcev1.NewVoicePeer(shc).AudioBufferClear(
		ctx,
		&babelforcev1.AudioBufferClearRequest{},
	); err != nil {
		return fmt.Errorf("typed browser barge-in: %w", err)
	}
	for range 50 {
		if _, err := shc.AudioStream().Write(toneFrame(sequence)); err != nil {
			return fmt.Errorf("write sustained browser audio: %w", err)
		}
		sequence++
		time.Sleep(20 * time.Millisecond)
	}
	if err := babelforcev1.NewApplicationEvents(shc).AudioSpeechStarted(
		ctx,
		&babelforcev1.AudioSpeechStartedEvent{Origin: "sender"},
	); err != nil {
		return fmt.Errorf("typed browser audio event: %w", err)
	}
	select {
	case err := <-received:
		if err != nil {
			return err
		}
	case <-ctx.Done():
		return ctx.Err()
	}
	for shc.State() == rtvbp.SessionStateConnecting || shc.State() == rtvbp.SessionStateActive {
		select {
		case <-time.After(time.Millisecond):
		case <-ctx.Done():
			return ctx.Err()
		}
	}
	if shc.State() == rtvbp.SessionStateFailed {
		return errors.New("browser session failed")
	}
	return nil
}

func serve() error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	ready := make(chan rtvbp.SHC, 1)
	handler := applicationHandler(func(ctx context.Context, shc rtvbp.SHC) error {
		if err := shc.OpenAudio(ctx, audioFormat()); err != nil {
			return err
		}
		ready <- shc
		return nil
	})
	server := ws.NewServer(ws.ServerConfig{
		Addr:        "127.0.0.1:0",
		AudioFormat: audioFormat(),
	}, handler)
	if err := server.Listen(); err != nil {
		return err
	}
	defer server.Shutdown(context.Background())
	fmt.Println(server.URL())

	var shc rtvbp.SHC
	select {
	case shc = <-ready:
	case <-ctx.Done():
		return ctx.Err()
	}
	if err := exchangeAudio(shc, 1200, -2400); err != nil {
		return err
	}
	var final [1]byte
	if _, err := shc.AudioStream().Read(final[:]); err != nil && !errors.Is(err, io.EOF) {
		return fmt.Errorf("terminal read: %w", err)
	}
	return nil
}

func client(url string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	ready := make(chan rtvbp.SHC, 1)
	config := ws.ClientConfig{
		Dial:         ws.DialConfig{URL: url},
		AudioFormat:  audioFormat(),
		Subprotocols: []string{},
	}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		ws.Client(config),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{
			OnBegin: func(ctx context.Context, shc rtvbp.SHC) error {
				if err := shc.AcceptAudio(ctx); err != nil {
					return err
				}
				ready <- shc
				return nil
			},
		})),
	)
	done := session.Run(ctx)
	var shc rtvbp.SHC
	select {
	case shc = <-ready:
	case err := <-done:
		return fmt.Errorf("session ended before ready: %w", err)
	case <-ctx.Done():
		return ctx.Err()
	}
	request := bridge.NewPingRequest()
	response, err := babelforcev1.NewApplicationPeer(session).Ping(ctx, request)
	if err != nil || response.T0 != request.T0 {
		return fmt.Errorf("typed ping: response=%#v error=%w", response, err)
	}
	if err := exchangeAudio(shc, 1200, -2400); err != nil {
		return err
	}
	if _, err := babelforcev1.NewApplicationPeer(session).SessionTerminate(
		ctx,
		&babelforcev1.SessionTerminateRequest{Reason: "interop complete"},
	); err != nil {
		return fmt.Errorf("terminal request: %w", err)
	}
	select {
	case err := <-done:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

func applicationHandler(onBegin func(context.Context, rtvbp.SHC) error) rtvbp.SessionHandler {
	return applicationHandlerWithPing(onBegin, bridge.NewPingHandler())
}

func applicationHandlerWithPing(
	onBegin func(context.Context, rtvbp.SHC) error,
	ping rtvbp.RequestHandler,
) rtvbp.SessionHandler {
	return rtvbp.NewHandler(
		rtvbp.HandlerConfig{OnBegin: onBegin},
		ping,
		rtvbp.HandleTerminalRequest(func(
			context.Context,
			rtvbp.SHC,
			*babelforcev1.SessionTerminateRequest,
		) (*babelforcev1.EmptyResponse, error) {
			return &babelforcev1.EmptyResponse{}, nil
		}),
	)
}

func browserPingHandler(bargeReady chan<- struct{}) rtvbp.RequestHandler {
	return rtvbp.HandleRequest(func(
		ctx context.Context,
		_ rtvbp.SHC,
		request *babelforcev1.PingRequest,
	) (*babelforcev1.PingResponse, error) {
		if data, ok := request.Data.(map[string]any); ok && data["barge_ready"] == true {
			select {
			case bargeReady <- struct{}{}:
			default:
			}
		}
		inbound, ok := rtvbp.InboundRequest(ctx)
		if !ok {
			return nil, errors.New("missing inbound ping context")
		}
		t2 := time.Now().UnixMilli()
		return &babelforcev1.PingResponse{
			T0: request.T0, T1: inbound.ReceivedAt.UnixMilli(), T2: t2,
			OWD: t2 - request.T0, Data: request.Data,
		}, nil
	})
}

func exchangeAudio(shc rtvbp.SHC, sent, reply int16) error {
	if _, err := shc.AudioStream().Write(pcmFrame(sent)); err != nil {
		return fmt.Errorf("write audio: %w", err)
	}
	received := make([]byte, 320)
	if _, err := io.ReadFull(shc.AudioStream(), received); err != nil {
		return fmt.Errorf("read audio: %w", err)
	}
	if bytes.Equal(received, make([]byte, len(received))) {
		return errors.New("received silent audio")
	}
	if binary.LittleEndian.Uint16(received) != uint16(reply) {
		return fmt.Errorf("received unexpected sample %d", int16(binary.LittleEndian.Uint16(received)))
	}
	return nil
}

func audioFormat() rtvbp.MediaFormat {
	return rtvbp.MediaFormat{
		Encoding: "L16", SampleRate: 8_000, BitDepth: 16, Channels: 1,
		PTime: 20 * time.Millisecond,
	}
}

func pcmFrame(sample int16) []byte {
	frame := make([]byte, 320)
	for offset := 0; offset < len(frame); offset += 2 {
		binary.LittleEndian.PutUint16(frame[offset:], uint16(sample))
	}
	return frame
}

func toneFrame(sequence int) []byte {
	frame := make([]byte, 320)
	for sample := range 160 {
		position := sequence*160 + sample
		value := int16(math.Round(math.Sin(2*math.Pi*440*float64(position)/8_000) * 8_000))
		binary.LittleEndian.PutUint16(frame[sample*2:], uint16(value))
	}
	return frame
}
