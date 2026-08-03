# RTVBP — roadmap & status

The big picture: what's delivered, what's next, and the epics that group related stories. The
operational detail lives on the [board](stories/README.md) (generated from story frontmatter); this
document is the hand-written narrative around it.

## Status

_As of 2026-08-03:_ the repository is being converted from a documentation site into the **home of
the protocol** — spec, generator, and SDKs. The frozen `babelforce.v1` authority is source-pinned;
the typed Rust catalog reproduces it byte-for-byte; and the generator now emits Go payloads, role
glue, the classic.v1 envelope codec, the published protocol reference, and language-neutral
conformance vectors. The Go SDK runs on the semantic session, negotiated audio, and WebSocket
transport seams delivered by R-8/R-9, and its memory-transport harness executes the generated
scenarios for both roles. Live WebSocket sessions now pass in both directions against published
`rtvbp-go v0.37.2`; R-15's real-call acceptance remains before R-16 tags the first Go SDK release.
The Docusaurus site lives under [`website/`](../website), leaving `docs/` for contributor material
and this backlog.

## Delivered

- The published prose specification for protocol v1 and its Docusaurus site
  (<https://babelforce.github.io/rtvbp/>) — now the narrative layer that the generated reference will
  grow around.

## Next

Milestone 1 is **spec + generator + a Go SDK at wire parity**, in the order on the
[board](stories/README.md). The through-line: capture the current bytes as frozen fixtures, make the
spec reproduce them, then generate everything else and prove it against those fixtures and against a
live `rtvbp-go v0.37` peer.

After M1, in order: **WebRTC audio with WebSocket control** (the pressing transport need), then the
**Rust SDK** re-housed from `private-source.invalid`, then **QUIC and SIP** bindings.

## Epics

An **epic** is a themed group of stories with a shared design doc. Stories join an epic via the
`epic: <slug>` frontmatter field, where `<slug>` matches a design doc at `docs/designs/<slug>.md`.
Use `/track:epic` to start one.

### Spec catalog and generator core — [`spec-catalog`](designs/spec-catalog.md)

Stand up the Rust spec workspace, port `babelforce.v1` into it, and prove byte-equality with the
deployed wire *inside the spec crate* before any emitter exists. Also the generator skeleton and its
cheapest emitter (the `catalog.json` manifest), which forces the model to be complete. Everything
downstream depends on this proof.

### The Go SDK — [`go-sdk`](designs/go-sdk.md)

The first target and the parity benchmark: generated payload types, envelope codec, and role
dispatch, plugged into a hand-written runtime whose transport abstraction (a control channel plus
dynamic media channels) accommodates WebRTC, QUIC, and SIP without the catalog or the session
noticing. Also fixes the runtime semantics that are known-wrong today — serial dispatch, one
keepalive policy, honest termination — none of which changes a byte on the wire.

### Conformance, interop, and acceptance — [`conformance`](designs/conformance.md)

Make "the SDKs agree with the spec and with each other" a mechanical fact: golden fixtures captured
from today's implementation, generated test vectors and multi-message scenarios consumed by a thin
per-SDK harness, interop against the published `rtvbp-go v0.37`, and `rtvbp-openai` completing a real
call as the acceptance test. Closes with the CI drift gate.

### Generated documentation — [`docs-gen`](designs/docs-gen.md)

Reference documentation becomes a projection of the catalog, emitted into the Docusaurus site: per
operation, per event, and — most usefully — per role, so an integrator sees exactly what their side
must implement and what it may call. Hand-written narrative stays hand-written and links into it.

### Multi-catalog and profiles — [`multi-catalog`](designs/multi-catalog.md)

Prove the catalog-agnosticism claim cheaply: a throwaway `demo.v1` catalog emitted by the same
generator, served alongside `babelforce.v1` on one endpoint by subprotocol. If it needs a special
case anywhere in the runtime, the abstraction is wrong — which is the point of running the
experiment now rather than discovering it with the Rust SDK.

### Later — WebRTC, Rust SDK, QUIC and SIP

Not yet stories; the transport abstraction in [designs/go-sdk.md](designs/go-sdk.md) was designed
against them.

- **WebRTC audio + WebSocket control** — `pion/webrtc`; the WS connection stays the control channel,
  SDP and ICE ride the reserved `transport.*` namespace, and paired tracks become one timed duplex
  media channel. Nothing in the session or the catalog changes.
- **Rust SDK** — re-house `private-source.invalid/crates/rtvbp` as `sdk/rust`, replacing its hand-written
  `protov1.rs` with generated output from the same catalog and adding the request timeouts it lacks.
  The TypeScript port follows the same path.
- **QUIC and SIP** — QUIC gives a bidi control stream plus dynamic media streams; SIP maps a dialog
  to a session, RTP to media channels, and carries control as in-dialog `INFO`.
