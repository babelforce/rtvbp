# RTVBP Go SDK

The Go SDK for the Real-Time Voice Bridging Protocol. It provides the hand-written session runtime,
memory, WebSocket, and Pion WebRTC+WebSocket transports, session-owned audio buffering, and
generated catalog and envelope code.

The protocol specification is the source of truth. Files under `catalog/` and `envelope/` are
generated and must not be edited by hand.

## Install

The current public release candidate is:

```bash
go get github.com/babelforce/rtvbp/sdk/go@v0.1.0-rc.3
```

## WebSocket server

```go
package main

import (
	"context"
	"log"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
)

func main() {
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{})
	server := ws.NewServer(ws.ServerConfig{
		Addr: "127.0.0.1:8080",
		Path: "/rtvbp",
	}, handler)
	if err := server.Listen(); err != nil {
		log.Fatal(err)
	}
	defer server.Shutdown(context.Background())

	select {}
}
```

## WebSocket client session

```go
package main

import (
	"context"
	"log"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
)

func main() {
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		ws.Client(ws.ClientConfig{
			Dial: ws.DialConfig{URL: "ws://127.0.0.1:8080/rtvbp"},
		}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())

	if err := session.Close(context.Background()); err != nil {
		log.Fatal(err)
	}
	if err := <-done; err != nil {
		log.Fatal(err)
	}
}
```

`NewSession` requires an envelope explicitly. The generated `v1classic.Envelope` preserves the
frozen `babelforce.v1` wire format.

## Handlers

`NewHandler` combines typed request and event adapters. Requests and events run through one serial
dispatcher, while responses bypass it so a handler can safely make a nested request.

```go
handler := rtvbp.NewHandler(
	rtvbp.HandlerConfig{},
	rtvbp.HandleRequest(func(
		ctx context.Context,
		h rtvbp.SHC,
		req *MyRequest,
	) (*MyResponse, error) {
		return &MyResponse{}, nil
	}),
)
```

Use `HandleTerminalRequest` when the successful response must be flushed before the session closes.
For slow work, `SHC.DeferResponse` returns a one-shot, request-bound response handle.

## Audio

The session owns the duplex audio buffers. `SHC.AudioStream()` is an `io.ReadWriter` with
`ClearReadBuffer()` and `Format()`. Protocol adapters bind the negotiated channel using
`SHC.OpenAudio` or `SHC.AcceptAudio`. The runtime currently supports fixed-width L16 and writes
outbound frames in exact `Format().PTime` chunks.

WebSocket clients configure their local static audio channel with `ws.ClientConfig.AudioFormat`.
WebSocket servers that act as the voice side configure the corresponding server audio format;
application-side servers may leave it unset and bind the format selected during
`session.initialize`.

### Optional WebRTC audio

WebRTC is additive; it does not replace WebSocket binary audio. `ws.Client` selects the existing
`rtvbp.v1` binding. `webrtcws.Client` selects `rtvbp.webrtc.v1`, which keeps control on WebSocket and
carries PCMU over Pion WebRTC while exposing the same L16 byte stream to the session.

Use `webrtcws.AddToServer` to offer both bindings from one server and let each client choose at
connection setup. The existing [demo client](examples/rtvbp-demo-client) selects either binding
with `-audio-transport`, and the [demo server](examples/rtvbp-demo-server) serves both. See the
[binding guide](https://babelforce.github.io/rtvbp/docs/transports/webrtc-websocket/).

## Keepalive

Configure transport-native keepalive with:

```go
rtvbp.WithKeepalivePolicy(rtvbp.KeepalivePolicy{
	Interval:  5 * time.Second,
	Timeout:   2 * time.Second,
	MaxMisses: 3,
})
```

WebSocket uses protocol Ping/Pong frames. The catalog `ping` operation remains available for timing
measurements but is never run automatically.

## Custom transports

A transport supplies an opaque control channel and optional named media channels:

```go
type Transport interface {
	Control() rtvbp.ControlChannel
	AcceptMedia(context.Context) (rtvbp.MediaChannel, error)
	OpenMedia(context.Context, string, rtvbp.MediaFormat) (rtvbp.MediaChannel, error)
	Close(context.Context) error
}
```

`Close` must flush control sends admitted before teardown. A `TransportFactory` context bounds
construction only; the session owns and closes the returned transport.

## Repository layout

```text
sdk/go/
  bridge/babelforcev1/  hand-written v1 voice/telephony policy
  catalog/              generated payload types and role glue
  envelope/             generated wire codecs
  transport/memory/     in-process conformance transport
  transport/ws/         WebSocket control and static audio transport
  transport/webrtcws/   optional Pion audio + WebSocket control transport
  examples/             load test and demo applications
```

## Test

```bash
go test ./...
go test -race ./...
go vet ./...
```

The demo client and server are nested modules and are tested from their respective directories.
