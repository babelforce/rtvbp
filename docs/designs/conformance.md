# Design: conformance vectors, interop, and acceptance

**Status:** accepted · **Pillar:** Proof · **Stories:** R-1, R-11, R-12, R-15, R-16, R-17, R-19, R-20

## Why

"The SDKs agree with each other and with the spec" must be a mechanical fact, not a review opinion.
Today nothing proves the Go, Rust, and TypeScript ports implement the same protocol — and the Rust
port has already drifted. This epic makes conformance an **artifact of the same generator** that
produces the SDKs, so a protocol change automatically produces the tests that police it.

It also carries the compatibility guarantee: deployed peers speaking `rtvbp-go v0.37` must keep
working against anything we ship.

## Approach

### Golden fixtures — the authority (R-1)

A capture program pinned to `rtvbp-go v0.40.0` marshals canonical instances and presence/format
variants through the deployed production types into `conformance/babelforce.v1/golden/`. A second
capture pinned to the released Rust implementation owns the four additive browser events absent
from Go. Source-specific inventories keep that provenance explicit. Committed **frozen**: changing
an existing byte requires deliberate review, because a change here means a wire change.

The original Go set was captured **before the spec was written**, so the spec was authored against
observed bytes rather than remembered semantics. R-17 later hardened the set to 46 fixtures while
leaving all original bytes unchanged.

### Generated vectors (R-11)

```
conformance/babelforce.v1/
  golden/                     # frozen capture (above)
  payloads/<method>.json      # valid byte-exact samples + invalid samples w/ expected error class
  envelope/classic.v1/frames.json   # encode cases (input → exact bytes)
                                    # decode cases (bytes → expected kind/fields)
                                    # incl. discrimination-order and malformed cases
  scenarios/*.json            # multi-message exchange scripts
```

Scenarios are authored as **typed Rust** in the spec crate — so they cannot drift from the schemas —
and serialized by the emitter:

```json
{ "roles": { "a": "voice", "b": "application" },
  "steps": [
    {"from": "a", "kind": "request",  "method": "session.initialize", "params": {…}, "id": "$init"},
    {"from": "b", "kind": "response", "response": "$init", "result": {"audio_codec": {…}}},
    {"from": "a", "kind": "event",    "event": "session.updated", "data": {…}},
    {"from": "a", "kind": "event",    "event": "dtmf",            "data": {…}}
  ] }
```

`$name` binds a generated id. Matching is byte-exact for encode-side checks and structural (method +
params after id normalization) for messages the SDK originates.

M1 ships three scenarios: `initialize → updated → dtmf`; termination covering application-initiated
`call.hangup`, the supported voice→application `session.terminate`, and the reverse-direction 501;
and a `ping` RTT exchange.

### Per-SDK harness

Each SDK hand-writes one thin harness (~200 lines) that reads the committed vectors, plays the
scripted peer for one role over the **memory transport**, and asserts the side under test. This is
the "abstract e2e tests rolled into every SDK" mechanism: SDKs consume the vectors from the monorepo
path — no vendoring, no copies to drift.

### Interop (R-12)

Separate from vectors, because vectors only prove self-consistency: stand up the new SDK against the
**published `rtvbp-go v0.37.2`** over a real WebSocket, exercising both role directions. R-17's
pinned comparison proves byte equality for all 40 common fixtures and explicitly classifies the six
non-common fixtures; R-12's live interop then proves the behavioral contract. Together these show
deployed telephony peers are unaffected rather than assuming the two old releases speak identical
bytes.

### Acceptance (R-15)

`rtvbp-openai` — a real service that today pins `rtvbp-go v0.37.2` — ports on a branch. The port must
touch only import paths and constructor calls; if it needs more, the SDK's ergonomics regressed. A
real end-to-end phone call is the acceptance test.

### Drift gate (R-16)

CI runs `cargo test` → `task generate` → `git diff --exit-code` → `go test ./...` → docs build.
Generated output is committed (so `go get` works without the generator) and CI proves it is current.
Drift becomes unmergeable.

## Alternatives considered

- **Hand-written per-SDK test suites.** What exists today (`proto/protov1/handler_test.go` is the
  closest thing to a conformance suite). They rot independently and prove nothing across languages.
- **Vendoring vectors into each SDK.** Copies drift; the monorepo makes a shared path free.
- **Generating the harness too.** The harness is small, language-shaped glue; generating it would
  cost more than it saves. The *vectors* are what must be generated.

## Risks & open questions

- Scenario matching must normalize generated ids without becoming so loose it stops catching
  regressions; keep encode-side checks byte-exact.
- Interop against a published module version means CI needs network access to the Go proxy, or a
  vendored copy of v0.37 — decide when wiring CI.

## Acceptance / done

Golden fixtures frozen and green at all three layers (spec-side, generated-Go, envelope frames); the
four scenarios pass over the memory transport; interop against `rtvbp-go v0.37` passes in both role
directions; `rtvbp-openai` completes a real call on the new SDK; CI fails on any regenerated diff.
