# rtvbp-go

> 🔊 Real-Time Voice Bridging Protocol - Golang Implementation

A powerful Go library for building real-time voice communication applications with bidirectional audio streaming, session management, and flexible transport protocols.

---

## ✨ Features

- **🎯 Real-Time Audio Streaming**: Bidirectional audio communication with low latency
- **🔌 Multiple Transport Protocols**: WebSocket and direct channel support  
- **📞 Session Management**: Complete session lifecycle handling with state management
- **🎛️ IVR Integration**: Built-in support for Interactive Voice Response applications
- **📊 Event-Driven Architecture**: Comprehensive event and request/response system
- **🔧 Flexible Handler System**: Extensible handlers for custom business logic
- **📈 Load Testing**: Built-in tools for performance testing and benchmarking
- **🎵 Audio Processing**: Ring buffer-based audio handling with codec support

## 🚀 Quick Start

### Installation

```bash
go get github.com/babelforce/rtvbp-go
```

### Basic Server Example

```go
package main

import (
    "context"
    "log/slog"
    
    "github.com/babelforce/rtvbp-go"
    "github.com/babelforce/rtvbp-go/audio"
    "github.com/babelforce/rtvbp-go/transport/ws"
)

func main() {
    server := rtvbp.NewServer(
        ws.Server(ws.ServerConfig{
            Addr: ":8080",
        }),
        rtvbp.WithHandler(&MyHandler{}),
    )
    
    server.ListenAndServe(context.Background())
}

type MyHandler struct{}

func (h *MyHandler) OnBegin(ctx context.Context, hc rtvbp.SHC) error {
    hc.Log().Info("Session started", "session_id", hc.SessionID())
    return nil
}

func (h *MyHandler) OnRequest(ctx context.Context, hc rtvbp.SHC, req *proto.Request) error {
    // Handle incoming requests
    return nil
}

func (h *MyHandler) OnEvent(ctx context.Context, hc rtvbp.SHC, evt *proto.Event) error {
    // Handle incoming events
    return nil
}
```

### Basic Client Example

```go
package main

import (
    "context"
    
    "github.com/babelforce/rtvbp-go"
    "github.com/babelforce/rtvbp-go/transport/ws"
)

func main() {
    client := rtvbp.NewClient(
        ws.Client(ws.ClientConfig{
            URL: "ws://localhost:8080/ws",
        }),
        rtvbp.WithHandler(&MyClientHandler{}),
    )
    
    session, err := client.Connect(context.Background())
    if err != nil {
        panic(err)
    }
    
    // Use session for communication
    defer session.Close()
}
```

## 🏗️ Architecture

### Core Components

- **Session Management**: Handles connection lifecycle, state, and cleanup
- **Transport Layer**: Pluggable transport protocols (WebSocket, Direct)
- **Audio System**: Real-time audio streaming with ring buffer management
- **Protocol Layer**: Message serialization, requests, responses, and events
- **Handler System**: Extensible business logic integration

### Transport Protocols

#### WebSocket Transport
- Production-ready WebSocket implementation
- Automatic reconnection support
- Binary and text message handling
- Built-in ping/pong for connection health

#### Direct Transport  
- In-memory channel-based transport
- Perfect for testing and local development
- Zero network overhead

## 📦 Project Structure

```
rtvbp-go/
├── audio/              # Audio processing and ring buffer management
├── examples/           # Example implementations
│   ├── rtvbp-demo-server/    # Demo server application
│   ├── rtvbp-demo-client/    # Demo client with audio support
│   └── loadtest/             # Performance testing tools
├── proto/              # Protocol definitions and message types
│   └── protov1/        # Protocol version 1 implementation
├── transport/          # Transport layer implementations
│   ├── ws/             # WebSocket transport
│   └── direct/         # Direct channel transport
└── internal/           # Internal utilities
```

## 🔧 Advanced Usage

### Custom Audio Handling

```go
// Implement custom audio processor
type MyAudioHandler struct {
    // Your audio processing logic
}

func (a *MyAudioHandler) Read(p []byte) (n int, err error) {
    // Read audio data
    return len(p), nil
}

func (a *MyAudioHandler) Write(p []byte) (n int, err error) {
    // Process outgoing audio
    return len(p), nil
}
```

### Session Events and Requests

The protocol supports various built-in message types:

- **Application Control**: `application.move` for IVR navigation
- **Session Management**: `session.initialize`, `session.terminate`, `session.set`
- **Audio Control**: `audio.buffer.clear` for audio buffer management
- **Call Events**: `call.hangup` and telephony integration
- **Recording**: `recording.start`, `recording.stop`
- **Health Checks**: Built-in ping/pong mechanism

### Custom Transport

```go
// Implement your own transport
type MyTransport struct {
    // Transport implementation
}

func (t *MyTransport) Send(ctx context.Context, data rtvbp.DataPackage) error {
    // Send data through your transport
    return nil
}

func (t *MyTransport) Receive(ctx context.Context) (rtvbp.DataPackage, error) {
    // Receive data from your transport
    return rtvbp.DataPackage{}, nil
}
```

## 🧪 Testing

Run the test suite:

```bash
go test ./...
```

Run with coverage:

```bash
go test -cover ./...
```

Load testing:

```bash
cd examples/loadtest
go run main.go -connections 100 -duration 60s
```

## 📊 Performance

The library is designed for high-performance real-time applications:

- **Low Latency**: Optimized for minimal audio delay
- **High Throughput**: Supports hundreds of concurrent sessions
- **Memory Efficient**: Ring buffer-based audio processing
- **Scalable**: Pluggable transport and handler architecture

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## TODO

**Reliability / Failure Handling**

- If ping requests are not answered consider connection to be dead? -> terminate session, reconnect ?
- Session Re-establishment (due to websocket connection loss, or unanswered ping requests)
- On server shutdown, send reconnect request, then transport layer will disconnect and re-connect with retries
- Connect retries

**Transport**

- [ ] websocket reconnect
- [ ] test quic protocol -> benchmark

**Client**

- [ ] client session must end if server dies or closes the connection

---

*Built with ❤️ for real-time voice communication*
