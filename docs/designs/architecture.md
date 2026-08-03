# Design: RTVBP architecture — spec-first, any transport × any envelope

**Status:** accepted · **Pillar:** Spec · **Stories:** all of M1 (R-1 … R-18)

## Why

RTVBP exists today as **three hand-written ports of one prose specification**:

| Implementation | Where | Notes |
|---|---|---|
| Go | `babelforce/rtvbp-go` | The de-facto reference; most complete |
| Rust | `private-source.invalid/crates/rtvbp` | "so it can later stand alone as `rtvbp-rs`" |
| TypeScript | `private-source.invalid/sdk/typescript/src/rtvbp.ts` | Browser voice console |

plus ~650 lines of prose in this repo's Docusaurus site. (An earlier Rust spec crate lived here and
was deleted in July 2025; it already generated JSON Schema + AsyncAPI from `schemars` types, and
encoded an *incompatible earlier* wire revision — evidence for both the approach and the drift risk.)

Every protocol extension means hand-editing N implementations and then the docs. The Rust port has
already drifted additively (four events Go does not have). Nothing mechanically proves the three
agree, and nothing proves the docs describe any of them.

This design makes the payload catalog the single source of truth and generates everything derivable
from it, while making the **envelope** and the **transport** independently substitutable so peers can
choose their network protocol without changing the protocol.

## Approach

### The layer model

```
L4  Generated catalog (e.g. babelforcev1)
    typed payloads · role interfaces · dispatch glue · typed peer clients
    · reference docs · conformance vectors
─────────────────────────────────────────────────────────────────────────
L3  Session runtime (hand-written, per language)
    correlation + timeouts · serial dispatch · lifecycle · keepalive policy
    · middleware · media pump
─────────────────────────────────────────────┬───────────────────────────
L2a Envelope codec (GENERATED from            │ L2b Handler audio API
    EnvelopeSpec) — classic.v1 first;         │     io.ReadWriter +
    jsonrpc2 / cbor conceivable later         │     ClearReadBuffer + Format()
─────────────────────────────────────────────┴───────────────────────────
L1  Transport = ControlChannel + 0..n MediaChannels
    ws · memory (M1) → webrtc+ws · quic · sip (later)
```

Two rules keep the layers honest:

- **L3 never sees bytes with meaning.** It works in semantic `ControlFrame`s; the envelope codec is
  the only thing that knows the JSON.
- **L1 never sees methods or ids.** It moves opaque control bytes and media frames.

There is exactly one sanctioned exception: the reserved **`transport.*` method namespace**, used for
in-band transport signaling (WebRTC SDP/ICE). It is reserved across all envelopes and catalogs and is
documented as such, so no catalog may claim it.

### What is generated vs. hand-written

**Generated** (from the spec, by our generator): payload types, method/event name constants, role
interfaces and dispatch adapters, typed peer clients, **envelope codecs**, reference documentation,
conformance vectors and scenarios, and the `catalog.json` manifest.

**Hand-written** (genuinely not derivable): the session runtime, transports, the audio ring buffer,
and each SDK's thin conformance harness.

### Spec model

Payload types are ordinary `serde` + `schemars` structs. Operations, events, and roles are declared
in an explicit `catalog() -> Catalog` registry function — no proc-macro, no linker tricks. The
registry is where method↔role↔request↔response pairing, terminal-operation flags, and canonical
examples live, because that metadata does not belong on a payload type.

**Presence semantics are encoded in the type system**, because JSON Schema alone cannot express Go's
three wire behaviors — and getting this wrong silently changes bytes:

| Spec type | Wire behavior | Generated Go |
|---|---|---|
| `T` | always present | value, no `omitempty` |
| `Option<T>` | omitted when absent | `omitempty` |
| `Nullable<T>` | always serialized; `null` when absent | pointer, **no** `omitempty` |

`Nullable<T>` is load-bearing today: all three `SessionInitializeRequest.metadata` fields,
`SessionInitializeResponse.audio_codec`, and `SessionUpdatedEvent.audio_codec` emit `null` rather
than disappearing.

Field **declaration order is part of the contract** — Go marshals struct fields in declaration order,
so the spec's order is the wire's order.

### Generator

The generator binary depends on the catalog crates and walks the `Catalog` **value in process** — no
intermediate schema file to parse, so nothing is lost in translation (typed examples, presence
classes, envelope rules all survive). A `catalog.json` manifest is *emitted* as a committed artifact
for external tooling and for reviewing spec changes as diffs, but it is not the generator's input.

Pipeline: **load** (link catalogs) → **validate** (unique names, every op has a role, examples
round-trip their schemas) → **resolve** (shared named types, per-field wire plan, target-language
naming) → **emit** (pure `model → [(path, bytes)]` per emitter) → **write** (deterministic ordering,
DO-NOT-EDIT headers, gofmt).

Generated Go uses **idiomatic identifiers** (`DtmfEvent`, field `Application`) — wire names live only
in struct tags, so cleaning up names costs nothing on the wire.

### Byte-identity strategy

Three independent layers, all in CI:

1. **Golden fixtures** — captured once from the current `rtvbp-go`, committed frozen. These are the
   authority; changing one requires deliberate review.
2. **Spec-side test** — serializing the spec's canonical examples must byte-equal the fixtures. This
   pins the *spec* to the wire before any emitter exists.
3. **Generated-SDK tests** — per fixture: unmarshal → re-marshal → `bytes.Equal`, plus
   construct-from-code → marshal → equal.

Plus a **cross-version interop test**: the new SDK against the published `rtvbp-go v0.37` over a real
WebSocket, in both role directions.

### Versioning, profiles, negotiation

- Catalog id is `name.vMAJOR` (`babelforce.v1`). Additive changes stay in-major; anything
  wire-breaking forks a sibling catalog. Envelope ids (`classic.v1`) and transport profiles (`ws.v1`)
  version the same way.
- A **profile** is the triple *(transport, envelope, catalog)*. The legacy profile `rtvbp.v1` is
  *(ws.v1, classic.v1, babelforce.v1)*.
- Negotiation is **out-of-band at connection establishment** — for WebSocket, the subprotocol
  preference list (`Sec-WebSocket-Protocol: rtvbp.v1`), server selects and echoes. **Absence of a
  subprotocol means `rtvbp.v1`**, which keeps every deployed client working unchanged.
- The per-message `version:"1"` field stays exactly as it is. It is a `classic.v1` envelope constant,
  not a negotiation surface; we do not retrofit a handshake into v1.

## Alternatives considered

- **Neutral schema language (KDL/TOML/JSON) as the spec.** Symmetric across languages, but we'd have
  to build and maintain a schema language and validator before writing a line of protocol. Rust types
  give us a validator (the compiler) for free, and the deleted `rtvbp-spec` crate proves the shape.
- **Hand-authored JSON Schema.** Standard tooling, but verbose to author and unable to express role
  direction, terminality, or presence-vs-null without extensions anyway.
- **Trait-based operation registry** (`trait RequestPayload { const METHOD }`, as the Rust port has).
  Rust cannot enumerate trait impls, so an explicit list is required regardless — better to make that
  list the spec than to hide it. Those trait impls become *generated* SDK ergonomics instead.
- **Attribute proc-macro.** Most work, most opacity, for a catalog of 10 operations and 9 events.
- **Off-the-shelf OpenAPI/AsyncAPI generators.** Produce unidiomatic output and cannot express the
  byte-level quirks or the role model. We may emit AsyncAPI as an artifact; we won't consume it.
- **Keeping audio on the `Transport` interface as an `io.ReadWriter`** (today's Go design). It cannot
  express "no media", "two media streams", or timed/packetized media — so it fails WebRTC and QUIC.

## Risks & open questions

- **Byte-identity is the whole bet.** If the presence model misses a case, we silently break deployed
  peers. Mitigation: golden fixtures captured *first* (R-1), before the spec is written.
- **Generator scope creep.** Emitters are pure functions over the model; resist per-SDK special
  cases leaking into the model. When a target needs something, prefer a namespaced schema extension
  (`x-go-type`) over model surgery.
- **The `transport.*` reservation** is enforced by catalog validation; R-13 must also write it into
  the public spec before any integrator could plausibly try to claim that namespace.
- **Deferred-response API shape** (sentinel error vs. explicit handle) — decide when the session is
  rewritten (R-9).
- WebRTC codec policy (transcode opus↔L16 in the media pump vs. force PCMU/8000) — an M2 decision.

## Acceptance / done

The union of M1's stories: the spec generates a Go SDK that is byte-identical to `rtvbp-go` on the
wire, proven by golden fixtures at three layers and by interop against the published v0.37; the
published documentation and the conformance vectors are outputs of the same generator; a second
catalog (`demo.v1`) is served alongside `babelforce.v1` over one endpoint; `rtvbp-openai` runs a real
call against the new SDK; and CI fails on any drift between the spec and its generated output.
