---
sidebar_position: 1
---

# Go SDK quickstart

This path builds an application-role WebSocket endpoint. The current public release candidate is:

```bash
go get github.com/babelforce/rtvbp/sdk/go@v0.1.0-rc.1
```

An application must answer `ping`, negotiate audio in `session.initialize`, and acknowledge the
terminal `session.terminate` request. The SDK generates those typed payloads and dispatch adapters
from the protocol catalog.

```go
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
```

The complete [compile-tested quickstart](https://github.com/babelforce/rtvbp/tree/main/sdk/go/examples/quickstart)
selects the deployed L16/8 kHz codec, opens the session audio stream, discards inbound audio, and
shuts down cleanly. Replace the discard loop with your own duplex audio pipeline.

## Connect it

1. Expose the endpoint over TLS as `wss://…/rtvbp`.
2. Configure authentication before accepting the WebSocket upgrade. For babelforce Cloud, follow
   the [deployment JWT guide](../deployments/babelforce-cloud.md).
3. Leave the default WebSocket profile as `rtvbp.v1` unless both peers support another profile.
4. Implement the events and operations your application uses from the generated
   [application role](../reference/babelforce.v1/roles/application.mdx).

The [WebSocket binding](../transports/websocket.md) defines framing and close behavior. The generated
[initialization flow](../reference/babelforce.v1/flows/initialize-updated-dtmf.mdx) shows the first
control exchange.
