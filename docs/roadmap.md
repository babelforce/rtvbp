# RTVBP — roadmap & status

The big picture: what's delivered, what's next, and the epics that group related stories. The
operational detail lives on the [board](stories/README.md) (generated from story frontmatter); this
document is the hand-written narrative around it.

## Status

_As of 2026-08-14:_ the repository is the **home of the protocol** — spec, generator, parity Go and
Rust SDKs, and published documentation. The frozen `babelforce.v1` authority is source-pinned; both
SDKs reproduce it byte-for-byte and consume the same generated vectors and scenarios. Go and Rust
interoperate in both roles with published `rtvbp-go v0.37.2`, and current Go/Rust peers exchange
typed control plus non-silent duplex WebRTC media in both client/server directions. Stable
`sdk/go/v0.1.1` is public after the dummyphone hardening patch, and `sdk/rust/v0.1.0` is the first
public Rust release; both resolve from clean external consumers. R-16 now retains only the final
legacy release and repository archive. Component-scoped release notes, deterministic assets,
checksums, and provenance are in progress as R-30. The Docusaurus site lives under
[`website/`](../website), leaving `docs/` for contributor material and this backlog.

## Delivered

- The published prose specification for protocol v1 and its Docusaurus site
  (<https://babelforce.github.io/rtvbp/>) — a babelforce-branded integration entry point with
  audience-specific quickstarts around the generated reference.
- The additive Go WebRTC-audio binding: Pion PCMU media with classic control on WebSocket, selectable
  independently from the preserved plain WebSocket-audio binding.
- Stable `sdk/go/v0.1.1`, after live service acceptance, dummyphone hardening, and public
  module-proxy verification.
- The Rust SDK: generated catalogs, roles and envelope plus a Tokio runtime, WebSocket and
  `rtvbp.webrtc.v1` transports, the voice/telephony bridge, both-role conformance, and cross-language
  Go interoperability, published as `sdk/rust/v0.1.0` and verified from a clean Cargo consumer.

## Next

Milestone 1 is **spec + generator + a Go SDK at wire parity**, in the order on the
[board](stories/README.md). The through-line: capture the current bytes as frozen fixtures, make the
spec reproduce them, then generate everything else and prove it against those fixtures and against a
live `rtvbp-go v0.37` peer.

The [WebRTC epic](designs/webrtc.md) and [Rust SDK epic](designs/rust-sdk.md) have established and
released the second audio binding and the second full language implementation. Only retirement of
the legacy Go repository remains from the release backlog; R-20 remains a bounded optional
system-acceptance follow-up. **QUIC, SIP, and TypeScript** are the next implementation families.

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

### WebRTC audio with WebSocket control — [`webrtc`](designs/webrtc.md)

Add a Pion WebRTC audio binding alongside—not in place of—the existing WebSocket-binary audio
binding. Both keep the authenticated WebSocket control path, and callers choose one at connection
setup. Reserved `transport.webrtc.*` offer/answer signaling uses the selected envelope before
catalog dispatch starts. PCMU makes the RTP side browser-compatible while the frozen v1 session API
continues to expose L16 PCM bytes.

### The Rust SDK — [`rust-sdk`](designs/rust-sdk.md)

The second complete SDK proves that the spec, generated role/envelope surfaces, runtime contract,
and WebRTC profile are language-neutral. Rust can serve or consume either current profile in either
role and shares the mechanical conformance and cross-language release gate with Go.

### Releases — [`releases`](designs/releases.md)

Version and publish the Go SDK, Rust SDK, and protocol snapshot independently while retaining one
source-of-truth repository. Release material is deterministic, tied to an immutable component tag,
checksummed, attestable, and described by component-owned notes.

### Later — TypeScript, QUIC and SIP

- **TypeScript** follows the same generated-surface path as Go and Rust.
- **QUIC and SIP** — QUIC gives a bidi control stream plus dynamic media streams; SIP maps a dialog
  to a session, RTP to media channels, and carries control as in-dialog `INFO`.
