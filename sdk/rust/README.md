# RTVBP Rust SDK

The Rust SDK is the Tokio implementation of the Real-Time Voice Bridge Protocol. It has role,
wire, runtime, WebSocket, and WebRTC parity with the Go SDK. This monorepo's specification and
generator are its only implementation source of truth.

The crate contains two deliberate layers:

- `zz_generated_*` catalog types, validation, role traits/adapters, typed peers, event emitters,
  envelope codecs, and fixture tests. Change the specification and regenerate; never edit these
  files.
- hand-written session, audio, transport, and `babelforce.v1` voice-bridge code for behavior that
  the protocol specification cannot derive.

## Install

The monorepo tags this package independently as `sdk/rust/v0.x.y`:

```toml
[dependencies]
rtvbp = { git = "https://github.com/babelforce/rtvbp", tag = "sdk/rust/v0.1.0" }
```

Rust 1.88 or newer is required. The SDK uses Tokio and `async` generated role traits.

## Run the examples

The application-role [quickstart](examples/quickstart.rs) offers both `rtvbp.v1` WebSocket audio
and `rtvbp.webrtc.v1` WebRTC audio on `127.0.0.1:8080`:

```sh
cargo run --manifest-path sdk/rust/Cargo.toml --example quickstart
```

The transport-level demo runs as a pair and selects either binding without changing its session
code:

```sh
# Terminal 1
cargo run --manifest-path sdk/rust/Cargo.toml --example dual_profile -- server webrtc

# Terminal 2; use the URL printed by the server
cargo run --manifest-path sdk/rust/Cargo.toml --example dual_profile -- client webrtc ws://127.0.0.1:PORT
```

Use `websocket` in both commands to select binary WebSocket audio. No credential is embedded. The
client optionally reads the complete HTTP authorization value from `RTVBP_AUTHORIZATION`.
WebRTC STUN/TURN configuration comes from `RTVBP_ICE_SERVERS`, `RTVBP_ICE_USERNAME`, and
`RTVBP_ICE_CREDENTIAL`.

The complete integration guide is in the published [Rust SDK quickstart](../../website/docs/getting-started/rust.md).

## Supported bindings

- `rtvbp.v1`: classic JSON control in WebSocket text messages and L16 audio in binary messages.
- `rtvbp.webrtc.v1`: the same control path plus non-trickle WebRTC PCMU media, exposed to callers
  as L16/8000/16-bit/mono/20 ms audio.

Both support client and server construction, headerless v1 compatibility, transport Ping/Pong,
typed control, terminal-response flush, and orderly shutdown. Current WebRTC limits are one audio
stream, PCMU only, no renegotiation, no trickle ICE, and no ICE restart.

## Migrating from the ancestor Rust crate

Replace hand-written `proto`/`protov1` payloads with `rtvbp::catalog::babelforcev1`, construct
handlers with the generated role adapter functions, and call peers/events through the generated
typed clients. Replace its byte transport with `transport::ws` or `transport::webrtcws`, and let
`Session` own the audio buffers and lifecycle. The ancestor is compatibility evidence only; do not
copy its wire types or use it as an implementation source.
