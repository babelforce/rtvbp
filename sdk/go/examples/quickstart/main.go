// Command quickstart is the smallest complete babelforce.v1 application endpoint.
package main

import (
	"context"
	"fmt"
	"io"
	"log"
	"os/signal"
	"syscall"

	"github.com/babelforce/rtvbp/sdk/go"
	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
)

type application struct{}

func (*application) SessionInitialize(
	ctx context.Context,
	shc rtvbp.SHC,
	request *v1.SessionInitializeRequest,
) (*v1.SessionInitializeResponse, error) {
	codec, err := selectCodec(request.AudioCodecOfferings)
	if err != nil {
		return nil, err
	}
	format, err := v1bridge.MediaFormat(codec, v1bridge.DefaultPTime)
	if err != nil {
		return nil, err
	}
	if err := shc.OpenAudio(ctx, format); err != nil {
		return nil, fmt.Errorf("open negotiated audio: %w", err)
	}

	// Replace this discard loop with the application's duplex audio pipeline.
	go func() {
		_, _ = io.Copy(io.Discard, shc.AudioStream())
	}()
	return &v1.SessionInitializeResponse{AudioCodec: codec}, nil
}

func (*application) SessionTerminate(
	context.Context,
	rtvbp.SHC,
	*v1.SessionTerminateRequest,
) (*v1.EmptyResponse, error) {
	return &v1.EmptyResponse{}, nil
}

func selectCodec(offerings []v1.AudioCodec) (*v1.AudioCodec, error) {
	want := v1bridge.AudioCodecL16_8kHzMono
	for index := range offerings {
		codec := &offerings[index]
		if codec.ID == want.ID && codec.Name == want.Name && codec.SampleRate == want.SampleRate &&
			codec.BitDepth == want.BitDepth && codec.Channels == want.Channels {
			return codec, nil
		}
	}
	return nil, fmt.Errorf("required codec %s was not offered", want.ID)
}

func main() {
	application := &application{}
	handler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{},
		v1bridge.NewPingHandler(),
		rtvbp.HandleRequest(application.SessionInitialize),
		rtvbp.HandleTerminalRequest(application.SessionTerminate),
	)
	server := ws.NewServer(ws.ServerConfig{
		Addr: "0.0.0.0:8080",
		Path: "/rtvbp",
	}, handler)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	errors := make(chan error, 1)
	go func() { errors <- server.Listen() }()

	select {
	case <-ctx.Done():
		if err := server.Shutdown(context.Background()); err != nil {
			log.Fatal(err)
		}
	case err := <-errors:
		if err != nil {
			log.Fatal(err)
		}
	}
}
