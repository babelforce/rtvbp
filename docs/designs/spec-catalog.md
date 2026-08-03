# Design: Spec crate and generator core

**Status:** accepted · **Pillar:** Spec · **Stories:** R-2, R-3, R-4, R-5, R-18

## Why

Nothing can be generated until a source of truth exists that is provably equal to the current wire.
This epic builds the spec workspace, ports `babelforce.v1` into it, and stands up the
generator skeleton — with byte-equality to the deployed protocol proven *inside the spec crate*,
before any language emitter exists. Everything downstream depends on that proof.

See [architecture.md](architecture.md) for the layer model, presence semantics, and the three-layer
byte-identity strategy this epic implements.

## Approach

### Workspace layout

```
spec/
  Cargo.toml                        # Rust workspace
  crates/
    rtvbp-spec-model/               # Catalog, Operation, Event, Role, Nullable<T>,
                                    #   EnvelopeSpec + classic.v1 reference codec
    rtvbp-spec-babelforce-v1/       # payload structs + catalog() + examples + scenarios
    rtvbp-spec-demo-v1/             # second catalog (R-14)
    rtvbp-spec-gen/                 # bin: emitters + CLI
  manifests/babelforce.v1.catalog.json   # GENERATED artifact
conformance/babelforce.v1/
  golden/                           # FROZEN capture from rtvbp-go — the authority
  payloads/ envelope/ scenarios/    # GENERATED
```

### Model sketch

```rust
pub struct Catalog { id: CatalogId /* babelforce.v1 */, operations: Vec<Operation>, events: Vec<Event> }

pub struct Operation {
    method: &'static str,          // "session.initialize"
    handled_by: Option<Role>,      // validated, then resolved to Voice | Application | Both
    request: TypeRef, response: TypeRef,
    terminal: bool,                // closes the session after replying
    docs: Option<&'static str>, examples: Vec<Example>,
}
pub struct Event { name: &'static str, emitted_by: Option<Role>, data: TypeRef, /* … */ }
```

Registration is one hand-maintained table — the *whole* role/direction model, nothing hidden in
attributes:

```rust
pub fn catalog() -> Catalog {
    Catalog::new("babelforce", 1)
        .operation(Operation::new::<SessionInitializeRequest, SessionInitializeResponse>(
            "session.initialize", Role::Application)
            .example(examples::session_initialize()))
        .operation(Operation::new::<CallHangupRequest, EmptyResponse>(
            "call.hangup", Role::Voice).terminal())
        .operation(Operation::new::<PingRequest, PingResponse>("ping", Role::Both))
        // …
        .event(Event::new::<DtmfEvent>("dtmf", Role::Voice))
}
```

Payload types are plain `#[derive(Serialize, Deserialize, JsonSchema)]` structs with `///` doc
comments (schemars carries them into schema descriptions, and the emitters into Go doc comments and
MDX prose). Presence follows the `T` / `Option<T>` / `Nullable<T>` table in
[architecture.md](architecture.md). Rare target hints use namespaced extensions
(`#[schemars(extend("x-go-type" = "int"))]`) — today needed because `AudioCodec.sample_rate` is Go
`int` while `DtmfEvent.pressed_at` is `int64`.

### The catalog to port

Operations — voice→application: `session.initialize`, `session.terminate`. Application→voice:
`session.set`, `session.get`, `application.move`, `call.hangup`, `audio.buffer.clear`,
`recording.start`, `recording.stop`. Both: `ping` (an ordinary catalog operation, not a framework
concern).

Events — voice: `session.updated`, `dtmf`, `call.hangup`, `audio.info`. Application:
`audio.speech.started`, plus the four the Rust port added for browser voice
(`output.transcript.delta`, `output.transcript.done`, `input.transcript`, `agent.tool.call`).

Envelope `classic.v1` (an `EnvelopeSpec`, not a catalog concern): flat JSON, constant
`version:"1"`, structural discrimination in the order **event → method → response**, responses carry
no id of their own (correlation via `response`), and `error.data` serializes under the key
**`"any"`** — a wire-visible typo that is now contract.

### Byte-parity findings

R-4's bidirectional proof covers all 29 frozen fixtures and pinned these emitter requirements:

- Struct declaration order is wire order. Open maps remain bare (notably `session.get`) and their
  canonical keys follow Go's lexical map-key ordering.
- `SessionInitializeRequest.metadata`, `SessionInitializeResponse.audio_codec`, and
  `SessionUpdatedEvent.audio_codec` are required nullable fields; `Option<T>` fields are omitted.
- Go serializes an integral `float64` such as `AudioInfoItem.bytes_per_second == 0` as `0`, not
  serde's default `0.0`; the spec carries a compatibility serializer for that field. Its exact
  supported deployed-rate envelope is positive zero or a finite non-negative value in
  `1e-5..=2^53`. The lower limit is where serde_json and Go both use fixed notation; through the
  upper limit every integer remains exactly representable. Values outside that envelope are not
  promised byte parity: [source-pinned Go witnesses](../../conformance/babelforce.v1/authority/)
  cover the notation mismatch at `1e-6 <= value < 1e-5`, possible shorter rounded integer
  spellings above `2^53`, negative zero, and non-finite values. Some disconnected outside values,
  including `1e-7` and the next `float64` above `2^53`, currently happen to match but are
  deliberately not compatibility commitments.
- The five native Go `int` fields carry `x-go-type: int`; timestamp and counter fields that are
  actually `int64` do not.
- `classic.v1` omits nil request params, correlates responses only through `response`, and keeps the
  event → method → response precedence and `error.data` → `"any"` override described above.

The only byte mismatch uncovered while establishing the proof was fixed in the spec's float
serialization; no frozen fixture was changed.

R-17 expanded the authority set without changing those original bytes: four additive Rust event
payloads and thirteen Go-derived presence/format variants bring the inventory to 46. This exposed
two additional requirements. `classic.v1` must preserve an explicitly present `result:null` rather
than normalizing it to an absent result, and `serde_json` must enable `float_roundtrip` so parsing
and re-emitting Go's fractional `float64` spelling is exact. The full inventory now pins every
optional payload field absent, request params present, errors without `any`, and the deployed -1,
400, 500, and 501 code spellings.

R-19 adds two source-pinned `output.transcript.done.text` variants, bringing the inventory to 48.
The released producer sends `text: None`, while its public serializer permits both non-empty and
empty present strings. The field therefore keeps Rust `Option<String>` semantics and carries the
narrow `x-go-type: "*string"` hint: generated Go uses `*string` with `omitempty` so absence,
present-empty, and present-nonempty remain byte-distinct.

### Frozen semantic constraints

R-18 resolves the semantics that bytes alone do not state:

- `session.terminate` is handled by the application. Deployed voice clients call it and deployed
  application handlers answer it; the reverse application→voice request deliberately receives 501.
- A response may contain both `result` and a valid `error`, or neither. This matches deployed
  validation rather than importing JSON-RPC exclusivity: both is treated as an error by the runtime;
  neither is a successful response with no result.
- Error code `0` and an empty message are rejected on encode and decode. Any other signed integer is
  accepted; `-1`, `400`, `500`, and `501` are documented conventions, not a closed enum.
- The reference codec preserves explicit null for lossless wire proof (`result:null` and `any:null`).
  The deployed Go decoder is lossy for those interface values and omits them if re-encoded. The
  frozen exception is a top-level `error:null`: it decodes as no error and re-encodes without the
  `error` key, matching the deployed Go codec's nil error pointer with `omitempty`.
- Catalog operation methods cannot claim `transport.*`; validation reserves that method namespace
  for envelope-independent transport signaling. Event names such as `transport.state` remain
  legal because events do not claim a control method. R-13 publishes the same operation-only rule
  for integrators.

### Generator skeleton

CLI `rtvbp-spec-gen --emit=<manifest|go|docs|vectors> --out=<dir>`, plus a `--check` mode used by CI.
Stages are load → validate → resolve → emit → write, as in [architecture.md](architecture.md). The
manifest emitter comes first: it is the cheapest emitter and it forces the model to be complete
before any language-specific work starts.

The authored catalog is the unresolved model: operation and event roles are optional there so the
validation stage can report omissions together with duplicate names, missing metadata, and invalid
examples. Resolution converts that state to emitter-facing operations and events with required
roles, stable name ordering, and a shared schema registry. Local `#/$defs/…` references are rewritten
to `#/schemas/…`; conflicting definitions with the same name fail resolution.

Emitters are pure functions returning relative paths and bytes. The manifest emitter returns
`<catalog-id>.catalog.json`; `--out` selects its destination directory, while manifest output defaults
to `spec/manifests`. The writer is the only filesystem-mutating stage. `--check` performs the same
pipeline and compares bytes without writing; bare `--check` checks every registered emitter at its
canonical destination (only the manifest in R-5) when invoked from the repository root. Each target
declares which output paths it owns so stale generated files are detected and synchronized without
touching handwritten files in mixed output trees.

The manifest is deterministic pretty JSON with a generated-file notice and one trailing newline. It
contains a format version, catalog id/name/major, sorted operations and events, roles, operation
terminality, documentation, canonical examples, schema references, and the sorted embedded schema
registry. Events do not have terminality in the protocol model, so only operations carry that flag.

## Alternatives considered

- **Write the spec first, capture fixtures later.** Rejected: the fixtures are the authority, and
  authoring the spec against remembered semantics is exactly how the Rust port drifted. Capture
  first, then make the spec match it.
- **Serialized JSON Schema as the generator's input.** Loses typed examples and presence classes, and
  adds a parser to maintain. Emitted as an artifact instead.

## Risks & open questions

- The presence/ordering subtleties are the risk; R-4 exists to surface them before emitters are
  written. Expect the spec to change shape during R-4 — that is the point.
- `session.get` returns a bare open map as its `result` rather than a wrapped object. The spec must
  model that as-is; whether a future catalog wraps it is not an M1 question.

## Acceptance / done

`cargo test` in `spec/` serializes every canonical example byte-identically to the frozen golden
fixtures, the catalog validates (unique names, every operation and event has a role, every example
round-trips its schema), and `rtvbp-spec-gen --emit=manifest` produces a deterministic
`catalog.json`.
