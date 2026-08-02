# Design: Spec crate and generator core

**Status:** accepted · **Pillar:** Spec · **Stories:** R-1, R-2, R-3, R-4, R-5

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
    handled_by: Role,              // Voice | Application | Both
    request: TypeRef, response: TypeRef,
    terminal: bool,                // closes the session after replying
    docs: Option<&'static str>, examples: Vec<Example>,
}
pub struct Event { name: &'static str, emitted_by: Role, data: TypeRef, /* … */ }
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

### Generator skeleton

CLI `rtvbp-spec-gen --emit=<manifest|go|docs|vectors> --out=<dir>`, plus a `--check` mode used by CI.
Stages are load → validate → resolve → emit → write, as in [architecture.md](architecture.md). The
manifest emitter comes first: it is the cheapest emitter and it forces the model to be complete
before any language-specific work starts.

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
