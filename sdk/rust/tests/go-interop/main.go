package main

import (
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/webrtcws"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
)

func main() {
	if len(os.Args) < 2 {
		panic("usage: go-interop server|client [url]")
	}
	var err error
	switch os.Args[1] {
	case "server":
		err = serve()
	case "client":
		if len(os.Args) != 3 {
			panic("client requires a WebSocket URL")
		}
		err = client(os.Args[2])
	default:
		panic("unknown mode " + os.Args[1])
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func serve() error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	ready := make(chan rtvbp.SHC, 1)
	handler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, shc rtvbp.SHC) error {
			if err := shc.OpenAudio(ctx, audioFormat()); err != nil {
				return err
			}
			ready <- shc
			return nil
		}},
		v1bridge.NewPingHandler(),
		rtvbp.HandleTerminalRequest(func(
			context.Context,
			rtvbp.SHC,
			*babelforcev1.SessionTerminateRequest,
		) (*babelforcev1.EmptyResponse, error) {
			return &babelforcev1.EmptyResponse{}, nil
		}),
	)
	server := ws.NewServer(webrtcws.AddToServer(ws.ServerConfig{
		Addr: "127.0.0.1:0",
	}, webrtcws.Config{}), handler)
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
	received := make([]byte, 320)
	if _, err := io.ReadFull(shc.AudioStream(), received); err != nil {
		return fmt.Errorf("Go server read Rust audio: %w", err)
	}
	if bytes.Equal(received, make([]byte, len(received))) {
		return errors.New("Go server received silent Rust audio")
	}
	if _, err := shc.AudioStream().Write(pcmFrame(-2400)); err != nil {
		return fmt.Errorf("Go server write audio: %w", err)
	}
	var final [1]byte
	if _, err := shc.AudioStream().Read(final[:]); err != nil && !errors.Is(err, io.EOF) {
		return fmt.Errorf("Go server terminal read: %w", err)
	}
	return nil
}

func client(url string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	ready := make(chan rtvbp.SHC, 1)
	websocket := ws.ClientConfig{Dial: ws.DialConfig{URL: url}}
	websocket.Subprotocols = []string{webrtcws.Subprotocol}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		webrtcws.Client(webrtcws.ClientConfig{WebSocket: websocket}),
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
		return fmt.Errorf("Go client ended before ready: %w", err)
	case <-ctx.Done():
		return ctx.Err()
	}
	request := v1bridge.NewPingRequest()
	response, err := babelforcev1.NewApplicationPeer(session).Ping(ctx, request)
	if err != nil || response.T0 != request.T0 {
		return fmt.Errorf("Go client typed ping: response=%#v error=%w", response, err)
	}
	if _, err := shc.AudioStream().Write(pcmFrame(1200)); err != nil {
		return fmt.Errorf("Go client write audio: %w", err)
	}
	received := make([]byte, 320)
	if _, err := io.ReadFull(shc.AudioStream(), received); err != nil {
		return fmt.Errorf("Go client read Rust audio: %w", err)
	}
	if bytes.Equal(received, make([]byte, len(received))) {
		return errors.New("Go client received silent Rust audio")
	}
	if _, err := babelforcev1.NewApplicationPeer(session).SessionTerminate(
		ctx,
		&babelforcev1.SessionTerminateRequest{Reason: "interop complete"},
	); err != nil {
		return fmt.Errorf("Go client terminal request: %w", err)
	}
	select {
	case err := <-done:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
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
