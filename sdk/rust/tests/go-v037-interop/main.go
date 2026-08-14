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

	rtvbp "github.com/babelforce/rtvbp-go"
	v1 "github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/babelforce/rtvbp-go/transport/ws"
)

const pingInterval = 20 * time.Millisecond

var audioProbe = pcmFrame(1200)

func main() {
	if len(os.Args) < 2 {
		panic("usage: go-v037-interop server|client [url]")
	}
	var err error
	switch os.Args[1] {
	case "server":
		err = serveApplication()
	case "client":
		if len(os.Args) != 3 {
			panic("client requires a WebSocket URL")
		}
		err = runVoice(os.Args[2])
	default:
		panic("unknown mode " + os.Args[1])
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func serveApplication() error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	initialized := make(chan struct{}, 1)
	updated := make(chan struct{}, 1)
	dtmf := make(chan struct{}, 1)
	terminated := make(chan struct{}, 1)
	audio := make(chan struct{}, 1)
	handler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{},
		v1.NewPingHandler(),
		rtvbp.HandleRequest(func(
			_ context.Context,
			shc rtvbp.SHC,
			request *v1.SessionInitializeRequest,
		) (*v1.SessionInitializeResponse, error) {
			if len(request.AudioCodecOfferings) == 0 {
				return nil, errors.New("missing codec offering")
			}
			selected := request.AudioCodecOfferings[0]
			go echoAudio(shc.AudioStream(), audio)
			signal(initialized)
			return &v1.SessionInitializeResponse{AudioCodec: &selected}, nil
		}),
		rtvbp.HandleRequest(func(
			_ context.Context,
			shc rtvbp.SHC,
			_ *v1.SessionTerminateRequest,
		) (*v1.EmptyResponse, error) {
			signal(terminated)
			go func() {
				time.Sleep(10 * time.Millisecond)
				_ = shc.Close(context.Background(), nil)
			}()
			return &v1.EmptyResponse{}, nil
		}),
		rtvbp.HandleEvent(func(context.Context, rtvbp.SHC, *v1.SessionUpdatedEvent) error {
			signal(updated)
			return nil
		}),
		rtvbp.HandleEvent(func(context.Context, rtvbp.SHC, *v1.DTMFEvent) error {
			signal(dtmf)
			return nil
		}),
	)
	server := ws.NewServer(ws.ServerConfig{Addr: "127.0.0.1:0"}, handler)
	if err := server.Listen(); err != nil {
		return err
	}
	defer server.Shutdown(context.Background())
	fmt.Println(server.URL())
	for name, event := range map[string]<-chan struct{}{
		"session.initialize": initialized,
		"session.updated":    updated,
		"dtmf":               dtmf,
		"audio":              audio,
		"session.terminate":  terminated,
	} {
		select {
		case <-event:
		case <-ctx.Done():
			return fmt.Errorf("published Go server waiting for %s: %w", name, ctx.Err())
		}
	}
	return nil
}

func runVoice(url string) error {
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	telephony := &telephony{ready: make(chan struct{})}
	voice := v1.NewClientHandler(telephony, &v1.ClientHandlerConfig{
		Call: v1.CallInfo{
			ID: "call", SessionID: "session", From: "100", To: "200",
		},
		App:          v1.AppInfo{ID: "application"},
		Metadata:     map[string]any{"interop": "rust"},
		PingInterval: pingInterval,
		SampleRate:   8000,
	}, func(_ context.Context, handler rtvbp.SHC) error {
		return exchangeAudio(handler.AudioStream())
	})
	session := rtvbp.NewSession(ws.Client(ws.ClientConfig{
		Dial: ws.DialConfig{URL: url}, PingInterval: pingInterval, SampleRate: 8000,
	}), rtvbp.WithHandler(voice))
	done := session.Run(ctx)
	select {
	case <-telephony.ready:
	case err := <-done:
		return fmt.Errorf("published Go voice ended before DTMF registration: %w", err)
	case <-ctx.Done():
		return ctx.Err()
	}
	telephony.sendDTMF("7")
	time.Sleep(80 * time.Millisecond)
	if err := voice.Terminate("interop complete"); err != nil {
		return err
	}
	select {
	case err := <-done:
		return err
	case <-ctx.Done():
		return ctx.Err()
	}
}

type telephony struct {
	dtmf  v1.TelephonyDtmfHandler
	ready chan struct{}
}

func (*telephony) Move(context.Context, *v1.ApplicationMoveRequest) (*v1.ApplicationMoveResponse, error) {
	return &v1.ApplicationMoveResponse{}, nil
}
func (*telephony) Hangup(context.Context, *v1.CallHangupRequest) error { return nil }
func (*telephony) SessionVariablesSet(context.Context, *v1.SessionSetRequest) error {
	return nil
}
func (*telephony) SessionVariablesGet(context.Context, *v1.SessionGetRequest) (map[string]any, error) {
	return map[string]any{}, nil
}
func (*telephony) RecordingStart(context.Context, *v1.RecordingStartRequest) (*v1.RecordingStartResponse, error) {
	return &v1.RecordingStartResponse{ID: "recording"}, nil
}
func (*telephony) RecordingStop(context.Context, string) error { return nil }
func (adapter *telephony) OnDTMF(handler v1.TelephonyDtmfHandler) error {
	adapter.dtmf = handler
	close(adapter.ready)
	return nil
}
func (*telephony) OnHangup(v1.TelephonyHangupHandler) error { return nil }

func (adapter *telephony) sendDTMF(digit string) {
	now := time.Now().UnixMilli()
	adapter.dtmf(&v1.DTMFEvent{Digit: digit, PressedAt: now, ReleasedAt: now + 1})
}

func echoAudio(stream io.ReadWriter, complete chan struct{}) {
	buffer := make([]byte, len(audioProbe))
	if _, err := io.ReadFull(stream, buffer); err != nil {
		return
	}
	if _, err := stream.Write(buffer); err != nil {
		return
	}
	if bytes.Equal(buffer, audioProbe) {
		signal(complete)
	}
}

func exchangeAudio(stream io.ReadWriter) error {
	if _, err := stream.Write(audioProbe); err != nil {
		return err
	}
	buffer := make([]byte, len(audioProbe))
	if _, err := io.ReadFull(stream, buffer); err != nil {
		return err
	}
	if !bytes.Equal(buffer, audioProbe) {
		return errors.New("audio probe changed")
	}
	return nil
}

func pcmFrame(sample int16) []byte {
	frame := make([]byte, 320)
	for offset := 0; offset < len(frame); offset += 2 {
		binary.LittleEndian.PutUint16(frame[offset:], uint16(sample))
	}
	return frame
}

func signal(channel chan struct{}) {
	select {
	case channel <- struct{}{}:
	default:
	}
}
