# Design: Go SDK — runtime and emitted glue

**Status:** accepted · **Pillar:** SDK · **Stories:** R-6, R-7, R-8, R-9, R-10, R-29

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

type MediaFormat struct {
    Encoding string
    SampleRate, Channels, BitDepth int
    PTime time.Duration
}
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

The envelope is a mandatory constructor dependency: `NewSession(env Envelope, opts ...Option)`.
The root runtime cannot import a generated envelope implementation to provide a default without an
import cycle. `Run` has one supervisor and exactly one terminal result. It moves the session to
`Connecting` before invoking the factory, starts the reader and dispatcher before `OnBegin` (so an
initialization hook can make a request), and reaches `Active` only after `OnBegin` succeeds. Every
exit passes through `Closing`; local close, context cancellation, and orderly transport EOF finish
at `Closed`, while factory, initialization, transport, keepalive, or close errors finish at
`Failed`.

The control reader is the sole caller of `Control().Recv` and `Envelope.Decode`. It stamps the
transport's `ReceivedAt`, resolves responses inline, and appends requests and events to an
unbounded mutex-backed FIFO. One dispatcher consumes that FIFO serially. A bounded dispatcher
channel is deliberately not used: a slow handler must never prevent the reader from receiving the
response to a nested or unrelated request. Pending requests use buffered one-result cells; shutdown
atomically detaches and resolves all of them with `ErrSessionClosed`, or with the failure cause for
a failed session.

#### Deferred and terminal responses

Deferred response ownership is explicit, not a sentinel error. Each inbound request receives a
request-scoped `SHC` whose reply state is one-shot. In addition to an ordinary `Respond`, it exposes
`RespondThenClose`, and `DeferResponse` returns a request-bound handle with the same response
operations. Claiming the handle suppresses the typed adapter's automatic response, preserves the
correlation id for background work, and lets the dispatcher continue. A duplicate response returns
`ErrResponseAlreadySent`; using the handle after shutdown returns `ErrSessionClosed`. If a handler
returns an error after deferring, the still-open handle is failed immediately rather than leaking.

`RespondThenClose` sends synchronously and then signals the supervisor without waiting from the
dispatcher. The supervisor invokes `Transport.Close` with an independent bounded context. The
transport's flush contract therefore makes the terminal response observable before connection
teardown without deadlocking the serial handler.

#### Keepalive ownership

The runtime owns one `KeepalivePolicy{Interval, Timeout, MaxMisses}` and the sentinel
`ErrKeepaliveTimeout`. A transport may implement the optional `KeepaliveTransport` monitor; when the
policy is enabled the session supervises it and treats a breach as a transport failure, moving to
`Failed` and resolving pending requests with the sentinel. Classic WebSocket implements the monitor
with protocol Ping/Pong frames through its sole writer. The catalog `ping` operation is never run
automatically.

### Audio API

Keep `io.ReadWriter` + `ClearReadBuffer` — it is what `rtvbp-openai` and every example consume, and
byte-oriented PCM is what AI backends want. Add `Format()`, plumbed from the `session.initialize`
negotiation. The session owns separate inbound and outbound ring buffers and pumps media frames in
and out, decoupling transport packet cadence from handler read cadence. Media binding is a
protocol-neutral session operation; the root session does not inspect `session.initialize` JSON.
The catalog adapter that selects or accepts the codec binds the resulting media channel and format.

For R-9 the byte pump supports fixed-width L16 explicitly. `MediaFormat` includes the negotiated
`BitDepth`, and the frame size is
`SampleRate * Channels * (BitDepth/8) * PTime / time.Second`, with positivity, integral-sample, and
overflow validation. Classic WebSocket uses a local 20 ms PTime default because babelforce.v1 does
not carry PTime on the wire. Variable-rate or otherwise unsupported encodings fail clearly instead
of guessing a packet size. The outbound pump writes exact PTime chunks and does not emit a partial
chunk during teardown. `ClearReadBuffer` drains inbound bytes without resetting or killing blocked
readers. Later, timed transports expose an optional `FrameAudio` interface on the same object for
callers that care about PTS; everyone else keeps reading bytes. `AudioObserver` survives unchanged
on the byte view.

### Generated glue

`rtvbp-spec-gen --emit=go` first converts the resolved catalog schemas into a Go-specific ordered
IR. Required fields are values, optional fields carry `omitempty`, and required nullable fields are
pointers without `omitempty`; schema `properties` order remains struct declaration order. Authored
schema names are preserved while wire field names are independently converted to Go identifiers,
including the `ID`, `RTT`, and `OWD` initialisms.

The source catalog also owns a target-neutral registry of all frozen payload/event fixtures. Each
entry binds a relative fixture path and bytes to an operation request, operation response, or event
type and is validated by typed round-trip before resolution. The Go emitter uses that registry to
generate standard-library-only construction and unmarshal/re-marshal tests for every registered
payload/event fixture; the `classic.v1` envelope fixtures remain R-7's responsibility.

Role API names describe the **local** role. `ApplicationHandler` implements operations handled by
the application, `ApplicationEventHandler` receives events emitted by the peer (the voice role), and
`ApplicationEvents` emits events owned by the application. `VoiceHandler`, `VoiceEventHandler`, and
`VoiceEvents` are the converse. An operation or event assigned to `Both` appears in both local-role
surfaces. This convention keeps an integrator's implementation, subscriptions, and allowed emits
under one role name instead of switching perspective between APIs.

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

// []any is intentional: these collections expand directly into NewHandler(...any).
func ApplicationHandlers(h ApplicationHandler) []any
func VoiceHandlers(h VoiceHandler) []any

type ApplicationEventHandler interface { // receives events emitted by Voice
    AudioInfo(...) ; CallHangup(...) ; Dtmf(...) ; SessionUpdated(...)
}
type ApplicationEvents struct{ /* narrow notifier */ } // emits Application events

// Typed client for the operations the *peer* role offers:
type VoicePeer struct{ /* … */ }      // used from the application side
func (v *VoicePeer) CallHangup(ctx context.Context, p *CallHangupRequest) (*EmptyResponse, error)
```

Typed peers depend on a narrow requester interface matching `Session.Request` and `SHC.Request`;
event emitters likewise depend on a narrow notifier matching `SHC.Notify`. They do not retain the
whole session handler context. A successful response with an absent payload is decoded as `{}`
before typed validation, matching the runtime's inbound empty-payload convention while still
allowing the generated response validator to reject an invalid empty value.

Validation is also generated, not handwritten into a catalog package. The target-neutral catalog
model carries structured validation metadata, and the Go emitter projects it into `Validate`
methods used by the runtime's existing `Validation` hook. The metadata includes every deployed
semantic constraint currently enforced by `proto/protov1`: required non-empty reasons and
identifiers, ping timestamp requirements, and DTMF digit, non-negative timestamp/sequence, and
`released_at >= pressed_at` cross-field ordering. Cross-field predicates are first-class structured
constraints rather than Go snippets, so later SDK emitters can produce the same behavior.

Per-role operation rejection is target-neutral catalog data for the same reason. In particular, the
voice role's reverse-direction `session.terminate` registration emits the deployed 501 code and
exact message from catalog metadata. A generic emitter must never recognize `babelforce.v1` or a
method spelling as a special case. Normal terminal behavior is simpler: the operation's `terminal`
flag alone selects `HandleTerminalRequest` and therefore `RespondThenClose`; handlers and peer
clients contain no per-operation shutdown side effects.

Unknown method ⇒ 501, unknown event ⇒ ignored (as today), both hookable. Identifiers are idiomatic
Go (`DtmfEvent`, field `Application`); wire names live only in tags.

The remaining `proto/protov1` orchestration is split by derivability. Generated payloads, role
adapters, typed peers, event helpers, validators, and role rejections replace and delete their
handwritten counterparts. Codec-to-`MediaFormat` conversion, the local 20 ms PTime policy, ping
timestamp calculation, initialization/audio negotiation, telephony callbacks, and audio
observation remain a small handwritten `babelforce.v1` voice bridge built on the generated API.
These are runtime or integration policies that the catalog does not describe. The nested demos move
to that bridge and generated types as part of the same cutover.

#### R-15 acceptance finding

The `rtvbp-openai` port exposed one convenience regression: a closure-oriented application can
register individual generated handlers, but without implementing the full role interface it had to
repeat the babelforce.v1 ping timestamp policy that `protov1.NewPingHandler` previously supplied.
The handwritten bridge now exposes `NewPingHandler`, backed by the same timing function as its voice
handler. Explicit `OpenAudio` after codec selection is intentional: media binding is runtime policy,
not catalog-derived behavior. With that helper restored, the consumer migration remains imports,
generated identifier changes, handler constructors, and the deliberate media/termination lifecycle
calls; no service structure changes are required.

#### Compatibility and failing-first sequence

The generated types already pin bytes; R-10 preserves behavior around those bytes. Structured
validators retain deployed request rejection instead of silently accepting zero values after the
type migration. Per-role rejection metadata retains the captured reverse `session.terminate` 501
without contaminating a catalog-agnostic emitter. Terminal metadata makes all three declared
terminal operations flush their response before close, and the narrow requester/notifier seams add
type safety without changing envelope or transport behavior.

Implementation proceeds with failing tests in this order:

1. Generator contract tests use a synthetic catalog containing Application, Voice, Both,
   terminal/non-terminal operations, both event directions, structured validators, and a per-role
   rejection. The expected role file fails before the emitter is extended.
2. Generated Go compile and adapter tests pin the exact interfaces, `[]any` registration, typed
   request/response conversion, empty-payload decoding, validation, event direction, rejection, and
   terminal close behavior.
3. Runtime integration tests pin default and hooked unknown method/event behavior and exercise the
   generated glue over the memory transport.
4. The legacy handler scenarios are ported to the generated API and voice bridge. Terminal
   `application.move` and `call.hangup` scenarios expect response-then-close rather than a later
   handwritten shutdown; both nested demos must compile and run against the new packages.
5. Delete the derivable `proto/protov1` types and adapters, regenerate, and run the full repository
   gate plus race and leak checks.

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
- Subtree import must not lose `rtvbp-go` history; verify before the old repo is archived.

## Acceptance / done

Generated types round-trip every golden fixture byte-identically; the session passes the conformance
scenarios over the memory transport; the WS transport interops with published `rtvbp-go v0.37` in
both role directions; examples and the load test run `goleak`-clean.
