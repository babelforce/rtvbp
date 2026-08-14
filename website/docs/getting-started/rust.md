---
sidebar_position: 2
sidebar_label: Rust SDK
description: Build either RTVBP role in Rust with generated contracts, a Tokio runtime, and WebSocket or WebRTC media.
---

# Rust SDK quickstart

The Rust SDK is the Tokio implementation of both RTVBP roles and both current audio bindings. Its
payloads, validation, role adapters, typed peers, event emitters, envelope codec, and conformance
tests are generated from this monorepo's protocol specification.

:::note Runtime boundary
Generated code defines payloads and role contracts. Tokio sessions, transports, and audio ownership
remain thin hand-written runtime layers behind those contracts.
:::

Add the independently tagged crate to a Rust 1.88-or-newer project:

```toml
[dependencies]
rtvbp = { git = "https://github.com/babelforce/rtvbp", tag = "sdk/rust/v0.1.0" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
async-trait = "0.1"
serde_json = "1"
webrtc = "=0.14.0" # needed when constructing ICE/TURN configuration
```

## Implement a role

An application implements the generated `ApplicationHandler` trait. The adapter turns the trait
into the exact request registrations declared by `babelforce.v1`; terminal flags and the frozen
reverse-role rejection are generated rather than repeated by application code.

```rust
use async_trait::async_trait;
use rtvbp::catalog::babelforcev1 as v1;
use rtvbp::{Error, HandlerContext};

struct Application;

#[async_trait]
impl v1::ApplicationHandler for Application {
    async fn ping(&self, ctx: HandlerContext, request: v1::PingRequest)
        -> Result<v1::PingResponse, Error> {
        // Use ctx.received_at() for t1 and the current epoch time for t2.
        todo!()
    }

    async fn session_initialize(&self, ctx: HandlerContext,
        request: v1::SessionInitializeRequest)
        -> Result<v1::SessionInitializeResponse, Error> {
        let selected = request.audio_codec_offerings.first().cloned();
        ctx.open_audio(rtvbp::bridge::babelforcev1::default_media_format()).await?;
        Ok(v1::SessionInitializeResponse { audio_codec: selected })
    }

    async fn session_terminate(&self, _: HandlerContext,
        _: v1::SessionTerminateRequest) -> Result<v1::EmptyResponse, Error> {
        Ok(v1::EmptyResponse(serde_json::Map::new()))
    }
}
```

The repository's [complete quickstart](https://github.com/babelforce/rtvbp/blob/main/sdk/rust/examples/quickstart.rs)
compiles this role, selects only the deployed L16/8 kHz offering, serves one session, and discards
inbound audio. Production servers accept in a loop and create one `Session` per accepted transport.

For the opposite role, `bridge::babelforcev1::VoiceBridge` supplies initialization, codec binding,
application timing ping, DTMF/hangup callbacks, terminal behavior, and optional audio counters over
the generated `VoiceHandler`. Implement only its `TelephonyAdapter` boundary.

## Generated clients, events, and audio

Use `ApplicationPeer` or `VoicePeer` for typed requests and `ApplicationEvents` or `VoiceEvents`
for typed events. They validate before encoding and decode the concrete generated response type.
`HandlerContext` supports the same calls inside a handler without deadlocking nested requests.

After `open_audio` or `accept_audio`, `context.audio()` exposes the session-owned duplex buffer:

- `read` receives peer audio and `write` sends audio;
- `clear_read_buffer` implements barge-in buffer clearing safely;
- `read_timed_frame` preserves RTP timestamps for WebRTC-aware consumers;
- `format` is immutable after negotiation.

Both bindings expose L16 little-endian, 8,000 Hz, 16-bit, mono audio in 20 ms / 320-byte frames.
The WebRTC transport converts that boundary to and from RTP PCMU.

## Choose WebSocket or WebRTC

Plain WebSocket audio uses `ws::ClientFactory`. WebRTC keeps the same authenticated WebSocket
control path and substitutes `webrtcws::ClientFactory`:

```rust
let websocket = rtvbp::transport::ws::ClientConfig::new(url);

// WebSocket text control plus binary L16 audio:
let factory = rtvbp::transport::ws::ClientFactory::new(websocket.clone());

// Or WebSocket text control plus WebRTC PCMU audio:
let factory = rtvbp::transport::webrtcws::ClientFactory::new(
    websocket,
    rtvbp::transport::webrtcws::Config::default(),
);
```

The compile-tested [dual-profile demo](https://github.com/babelforce/rtvbp/blob/main/sdk/rust/examples/dual_profile.rs)
runs in client or server mode and selects `websocket` or `webrtc`. It embeds no credentials.

## Authentication and ICE/TURN

Set `ClientConfig.authorization` to the complete bearer header value. On servers, set
`ServerConfig.authenticate` to validate the Upgrade request before the server emits `101` and,
for WebRTC, before it parses SDP or starts ICE. Token validation policy belongs to the deployment;
the babelforce policy is documented in the [deployment authentication guide](../deployments/babelforce-cloud.md).

WebRTC callers provide `webrtc::peer_connection::configuration::RTCConfiguration` through
`webrtcws::Config.peer_connection`. Same-host tests need no ICE server. Production normally uses
STUN and often TURN; load URLs, usernames, and rotating credentials from secret management. The
demo reads comma-separated `RTVBP_ICE_SERVERS` plus optional `RTVBP_ICE_USERNAME` and
`RTVBP_ICE_CREDENTIAL` and has no default public service.

## Shutdown and errors

Call `Session::close().await` for graceful client shutdown and `Server::shutdown().await` to stop
admission and close active WebSockets. Generated terminal handlers flush their correlated response
before orderly transport close. Dropping a task is not the shutdown API.

Configuration, validation, timeout, remote response, media, transport, and session failures are
distinct `rtvbp::Error` variants. A session finishes `Closed` after local close or orderly EOF and
`Failed` after factory, initialization, keepalive, transport, codec, or close failure.

## Current limits and migration

`rtvbp.webrtc.v1` currently supports one bidirectional `audio` stream, PCMU only, non-trickle ICE,
and no renegotiation, ICE restart, Opus, or packet-loss concealment. SDP is bounded to 512 KiB and
the complete signaling frame to 1 MiB.

When migrating from the ancestor Rust crate, replace its hand-written protocol module with
`catalog::babelforcev1`, generated role adapters, and typed peers/events. Replace its byte transport
with `transport::ws` or `transport::webrtcws`; let `Session` own correlation, timeouts, lifecycle,
and audio buffers. Earlier repositories are compatibility and migration evidence only. This
monorepo is the sole current RTVBP implementation source.
