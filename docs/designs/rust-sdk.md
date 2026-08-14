# Design: Rust SDK — generated protocol surfaces and runtime parity

**Status:** accepted · **Pillar:** SDK · **Stories:** R-25, R-26, R-27, R-28

## Why

The Go SDK proves the spec-first architecture, but one implementation is not enough to prove that
the catalog, envelope, role, session, and transport boundaries are genuinely language-neutral. The
repository also has a deployed Rust ancestor in a maintained production system; it is useful
migration evidence, but its hand-written payloads have already drifted from the frozen Go wire and
it lacks the current session, conformance, and WebRTC contracts.

This repository supersedes every previous RTVBP implementation. The Rust SDK therefore lives here,
is generated from the same spec as Go, and reaches behavioral parity with the Go v0.1 SDK rather
than preserving the architecture or API of the ancestor crate.

## Parity contract

Parity means that a Rust peer can take either role anywhere a Go peer can, with the same observable
wire, lifecycle, media, failure, and shutdown behavior.

| Surface | Rust parity requirement |
|---|---|
| Catalog | Generated payloads, validation, names, role traits, dispatch adapters, typed peers, and event emitters for every loaded catalog |
| Envelope | Generated `classic.v1` codec reproduces every frozen frame byte-for-byte, including structural precedence and null quirks |
| Session | Correlation and timeouts, response fast path, serial request/event dispatch, deferred replies, terminal flush, lifecycle, keepalive, middleware, and deterministic pending-request failure |
| Audio | Session-owned duplex buffering, exact negotiated frame sizing, clear-read-buffer, fixed-width L16, and timed-frame access |
| Transports | Memory, WebSocket control plus optional binary audio, and WebRTC audio plus WebSocket control |
| Profiles | `rtvbp.v1`, headerless legacy fallback, multi-catalog selection, and `rtvbp.webrtc.v1` |
| Proof | The same generated vectors/scenarios, frozen fixtures, live Go interop in both roles, and cross-language WebRTC acceptance |

Language syntax is not part of parity. Rust uses `async`/Tokio and Rust naming conventions; it does
not imitate Go channels, interfaces, or `io.ReadWriter` mechanically.

## Package and generated boundary

The SDK is one independently publishable crate:

```text
sdk/rust/
  Cargo.toml
  src/
    lib.rs                         hand-written public assembly
    catalog/
      babelforcev1/zz_generated_*  GENERATED payload and role surfaces
      demov1/zz_generated_*        GENERATED second-catalog proof
    envelope/
      v1classic/zz_generated_*     GENERATED codec
    session/                       hand-written runtime
    audio/                         hand-written duplex buffering
    bridge/babelforcev1/           hand-written codec/audio and telephony policy
    transport/{memory,ws,webrtcws}/ hand-written bindings
  tests/                           thin conformance and interop harnesses
```

The crate is named `rtvbp` and is tagged from the monorepo as `sdk/rust/v0.x.y`. Generated files are
committed, carry a DO-NOT-EDIT banner, and are owned exclusively by the Rust generator target.

Everything derivable follows the Go boundary exactly:

- catalog payload types, method/event identities, structured validation, role-local handler traits,
  handler adapters, typed peer request methods, event emitters, role rejections, and terminal flags;
- envelope codecs declared by `EnvelopeSpec`;
- fixture construction/round-trip tests and role-surface contract tests.

Session execution, transports, buffering, authentication policy, clock calculations, and codec
selection remain hand-written because the spec does not describe them.

### Presence and ordering

The Rust emitter preserves the same three presence classes and declaration order:

| Spec | Generated Rust | Serde behavior |
|---|---|---|
| `T` | `T` | required and always serialized |
| `Option<T>` | `Option<T>` | `skip_serializing_if = "Option::is_none"` |
| `Nullable<T>` | `Option<T>` | required key; `None` serializes as `null` |

Struct fields are emitted in schema property order. `serde_json` therefore writes payload members in
the frozen order. Open JSON objects use `serde_json::Map<String, Value>` so insertion order remains
observable where the protocol permits arbitrary keys.

## Runtime architecture

The Rust runtime implements the same semantic layer boundaries as Go:

- `ControlFrame` is envelope-independent; only an `Envelope` sees JSON bytes.
- `ControlChannel` moves opaque control bytes and timestamps.
- `MediaChannel` moves named, formatted media frames with optional PTS.
- `Transport` combines one control channel with zero or more media channels.
- `TransportFactory` receives the selected envelope so a composite transport may use the reserved
  `transport.*` namespace without leaking signaling into catalog dispatch.

Object-safe async traits use `async-trait`; the public runtime is Tokio-based. A session owns one
reader task and one serial dispatcher. Responses resolve pending requests directly on the reader;
requests and events enter an unbounded FIFO so a slow handler cannot block a nested response.
Shutdown atomically resolves every pending request exactly once.

The lifecycle and error classes match Go: connecting, active, closing, closed, and failed; local
close/orderly EOF are closed, while factory, initialization, keepalive, transport, codec, and close
errors are failed. Deferred response handles and terminal responses are one-shot. Transport close
must drain already-admitted control sends before closing the wire.

Audio is byte-oriented for ordinary consumers and frame-aware for timed consumers. The session
owns separate inbound/outbound buffers, emits only complete negotiated PTime chunks, drops a final
partial outbound chunk, and never lets `clear_read_buffer` poison a blocked reader.

## WebSocket binding

The `ws` transport provides client and server construction, authentication-before-upgrade seams,
subprotocol selection, protocol Ping/Pong keepalive, serialized writes, drain-before-close, text
control frames, and the optional legacy binary L16 audio channel. Absence of a subprotocol continues
to select `rtvbp.v1` where the server enables legacy compatibility.

## WebRTC plus WebSocket binding

The Rust binding is the same `webrtcws.v1` profile implemented by Go:

- WebSocket remains the authenticated semantic control channel;
- the selected WebSocket subprotocol is `rtvbp.webrtc.v1`;
- one envelope-encoded `transport.webrtc.offer` request and correlated answer carry complete,
  bounded non-trickle SDP;
- one send/receive audio transceiver negotiates only PCMU/8000/1;
- the SDK boundary is L16 little-endian, 8000 Hz, 16-bit, mono, 20 ms;
- each outbound SDK frame becomes one 160-byte PCMU media sample; inbound RTP produces timed L16
  frames with RTP-derived PTS;
- ICE servers, including STUN/TURN credentials, are caller configuration with no embedded public
  service or secret;
- failure, duplicate binding, unsupported media, cancellation, remote control close, and idempotent
  close have the same deterministic outcomes as Go.

The first implementation uses the stable `webrtc` 0.14 crate with Tokio. Alpha `webrtc-rs` releases
are deliberately excluded until parity is established. Opus, trickle ICE, ICE restart,
renegotiation, and multiple media streams require a later binding version and do not replace this
contract.

## Conformance and cross-language proof

The Rust harness remains thin: it consumes committed generated vectors and scenarios rather than
restating protocol cases in Rust. Completion requires:

1. every frozen payload and classic envelope fixture round-trips with exact bytes through generated
   Rust types and codec;
2. every generated valid/invalid vector and both-role scenario runs through a real Rust session;
3. Rust completes live WebSocket sessions in both roles against published `rtvbp-go v0.37.2`;
4. current Go and Rust complete typed control plus non-silent duplex media in both client/server
   directions over `rtvbp.webrtc.v1`, and the proof fails if media uses WebSocket binary frames;
5. cancellation, race-sensitive arbitration, leak-free shutdown, and terminal flush are exercised
   repeatedly under Tokio's multi-thread runtime.

The repository gate runs format, clippy with warnings denied, Rust SDK tests, generator drift, Go
tests, cross-language interop, and docs in one ordered chain.

## Migration rule

No source file is copied from an earlier repository without being reconciled against this design
and the generated protocol surfaces. In particular, the ancestor `proto.rs` and `protov1.rs` are
discarded, not imported. Useful session and WebSocket algorithms may be adapted only after parity
tests expose the Go contract they must satisfy.

## Acceptance

The union of R-25 through R-28: a committed, generated-and-hand-written Rust SDK can replace the Go
SDK on either side of every supported profile, including duplex Pion-compatible WebRTC audio, and
the complete repository gate proves wire, role, runtime, transport, and cross-language parity.
