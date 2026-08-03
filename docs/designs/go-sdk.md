# Design: Go SDK — runtime and emitted glue

**Status:** accepted · **Pillar:** SDK · **Stories:** R-6, R-7, R-8, R-9, R-10

## Why

The Go SDK is the first target and the parity benchmark for everything the generator produces.
It must speak the wire exactly as `rtvbp-go` does while being generated from the spec, and its
runtime must accommodate transports that do not exist yet (WebRTC, QUIC, SIP) without the catalog or
the session noticing.

This epic also cleans up runtime semantics that are known-wrong today. The wire is frozen; the
*behaviour inside a peer* is not.

See [architecture.md](architecture.md) for the layer model.

## Approach

### Module

`github.com/babelforce/rtvbp/sdk/go`, root package `rtvbp`. Tags `sdk/go/v0.x.y`; stays on v0 until
`rtvbp-openai` and the platform have ported, which avoids a `/vN` path suffix during churn. History
is imported from `rtvbp-go` by subtree so the runtime port is a diff, not a rewrite.

```
sdk/go/
  *.go                      # runtime: session, frame/envelope + transport interfaces, audio
  envelope/v1classic/       # GENERATED codec
  transport/{ws,memory}/    # hand-written; webrtcws · quic · sip later
  catalog/{babelforcev1,demov1}/   # GENERATED
  conformancetest/          # harness over ../../conformance
  examples/                 # separate module
```

### Interfaces

```go
type Kind uint8
const (KindRequest Kind = iota + 1; KindResponse; KindEvent)

// The envelope-independent semantic unit. L3 speaks this; only the codec knows JSON.
type ControlFrame struct {
    Kind       Kind
    ID         string          // request/event id ("" if the envelope has none)
    CorrelID   string          // response only: the request being answered
    Method     string          // request method or event name
    Payload    json.RawMessage // params | result | data
    Err        *WireError      // response only
    ReceivedAt time.Time
}
type WireError struct { Code int; Message string; Data json.RawMessage } // classic.v1 writes Data as "any"

type Envelope interface {   // GENERATED per EnvelopeSpec; stateless and pure
    Name() string
    Encode(ControlFrame) ([]byte, error)
    Decode([]byte) (ControlFrame, error)
}

type ControlChannel interface {
    Send(ctx context.Context, data []byte) error
    Recv(ctx context.Context) (Received, error)   // io.EOF on orderly close
}

type MediaFormat struct { Encoding string; SampleRate, Channels int; PTime time.Duration }
type MediaFrame  struct { Data []byte; PTS time.Duration; Timed bool }

type MediaChannel interface {
    ID() string                  // "audio" is the default duplex voice stream
    Format() MediaFormat
    WriteFrame(MediaFrame) error
    ReadFrame() (MediaFrame, error)
    Close() error
}

type Transport interface {
    Control() ControlChannel
    AcceptMedia(ctx context.Context) (MediaChannel, error)
    OpenMedia(ctx context.Context, id string, f MediaFormat) (MediaChannel, error) // ErrMediaUnsupported where N/A
    Close(ctx context.Context) error   // MUST flush queued control sends before teardown
}

// The envelope is passed in so composite transports can signal in the reserved transport.* namespace.
type TransportFactory func(ctx context.Context, env Envelope) (Transport, error)
```

How each binding degrades:

| Transport | `Control()` | Media |
|---|---|---|
| `ws` | text frames | one static duplex `"audio"`, binary frames, `Timed=false` |
| `memory` | channel pair | zero or one in-process channel |
| `webrtcws` | the WS connection | duplex `"audio"` from paired tracks, `Timed=true` |
| `quic` | one bidi stream, length-prefixed | dynamic streams with a small `{id, format}` header |
| `sip` | in-dialog `INFO`, `application/rtvbp+json` | RTP session(s), `Timed=true` |

### Session semantics — deliberate changes from today

| Concern | Today | New |
|---|---|---|
| Dispatch | one goroutine per inbound message; no ordering | responses resolved on the reader (never blocks); requests + events through **one serial dispatcher**. Nested requests still work because responses bypass the queue |
| Slow handlers | n/a | deferred-response escape hatch: don't auto-reply, answer later via `SHC` |
| Pending on close | dangle until timeout (Rust: forever — no timeout at all) | resolved with `ErrSessionClosed` |
| Termination | `OnAfterReply` side-effect hooks; reverse application→voice `session.terminate` gets 501 | `SHC.RespondThenClose` + the transport's flush-on-close guarantee; spec-level `terminal: true` drives the voice→application path; reverse application→voice requests preserve the deployed 501 |
| Keepalive | WS ping every 5s **and** app-level `ping` every 10s, no defined failure action | one `KeepalivePolicy{Interval, Timeout, MaxMisses}` per transport; breach ⇒ `ErrKeepaliveTimeout` ⇒ `Failed`, pending resolved, hooks run. Catalog `ping` remains an RTT/OWD *measurement*, no longer auto-run |
| Lifecycle | `inactive→active→closing→closed`/`failed` | adds `Connecting` (factory + `OnBegin` window) |
| Audio chunking | hardcoded 320 bytes; `ChunkSize` config is dead code | `Format().PTime` worth of bytes |

Correlation ids are minted by the session via an injectable `IDGen` (default nanoid, preserving the
current wire look); the envelope treats ids as opaque strings.

### Audio API

Keep `io.ReadWriter` + `ClearReadBuffer` — it is what `rtvbp-openai` and every example consume, and
byte-oriented PCM is what AI backends want. Add `Format()`, plumbed from the `session.initialize`
negotiation. The session owns the ring-buffer pair and pumps media frames in and out, decoupling
transport packet cadence from handler read cadence. Later, timed transports expose an optional
`FrameAudio` interface on the same object for callers that care about PTS; everyone else keeps
reading bytes. `AudioObserver` survives unchanged on the byte view.

### Generated glue

```go
// GENERATED — role asymmetry as concrete API surface
type ApplicationHandler interface {   // what the application side must implement
    SessionInitialize(ctx context.Context, h rtvbp.SHC, req *SessionInitializeRequest) (*SessionInitializeResponse, error)
    SessionTerminate(...) ; Ping(...)
}
type VoiceHandler interface {         // what the voice side must implement
    CallHangup(...) ; ApplicationMove(...) ; SessionSet(...) ; SessionGet(...)
    AudioBufferClear(...) ; RecordingStart(...) ; RecordingStop(...) ; Ping(...)
}

func ApplicationHandlers(h ApplicationHandler) []rtvbp.RequestHandler  // binds into the runtime
func VoiceHandlers(h VoiceHandler) []rtvbp.RequestHandler

// Typed client for the operations the *peer* role offers:
type VoicePeer struct{ /* … */ }      // used from the application side
func (v *VoicePeer) CallHangup(ctx context.Context, p *CallHangupRequest) (*EmptyResponse, error)
```

Unknown method ⇒ 501, unknown event ⇒ ignored (as today), both hookable. Identifiers are idiomatic
Go (`DtmfEvent`, field `Application`); wire names live only in tags.

## Alternatives considered

- **Keep `Transport{Write, ReadChan, Close}` + audio as `io.ReadWriter` on the factory.** Simplest,
  and it is what exists — but it hard-codes "exactly one untimed interleaved byte stream", which
  WebRTC and QUIC both violate. Rust's `Frame{Text,Binary}` has the same ceiling.
- **Generate the runtime too.** Session correlation, goroutine lifecycles and transport plumbing are
  genuinely language-shaped; generating them would produce worse code than writing it once per SDK.
- **Preserve behaviour exactly, fix semantics later.** Rejected by the user: the wire is the
  contract, not the bugs, and building the known-wrong version first means building it twice.

## Risks & open questions

- Serial dispatch changes timing for handlers that currently benefit from accidental concurrency; the
  deferred-response hatch must land with it, not after.
- Deferred-response API shape (sentinel error vs. explicit handle) — decide during R-9.
- Subtree import must not lose `rtvbp-go` history; verify before the old repo is archived.

## Acceptance / done

Generated types round-trip every golden fixture byte-identically; the session passes the conformance
scenarios over the memory transport; the WS transport interops with published `rtvbp-go v0.37` in
both role directions; examples and the load test run `goleak`-clean.
