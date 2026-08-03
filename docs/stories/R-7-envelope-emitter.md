---
id: R-7
title: EnvelopeSpec and the generated Go classic.v1 codec
pillar: Generator
status: done
priority: 8
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: classic.v1 is declared once and emitted as a byte-proven Go Envelope implementation
---

# EnvelopeSpec and the generated Go classic.v1 codec

## Goal
Make the envelope a described, generated artifact rather than hand-written per language — so a
second envelope costs a spec entry, and the `classic.v1` quirks are stated once instead of being
re-derived by every SDK author.

## Acceptance
- [x] `EnvelopeSpec` describes `classic.v1` completely: the constant `version:"1"`, the three frame
      kinds and their field names, structural discrimination in the order **event → method →
      response**, responses correlating via `response` with no id of their own, `params` omitted when
      nil, and the `error.data` → `"any"` key override.
- [x] `--emit=go` writes `sdk/go/envelope/v1classic/` implementing the runtime's `Envelope`
      interface (`Name`, `Encode`, `Decode`) entirely from that description.
- [x] The codec is generated — no hand-written encode/decode logic for `classic.v1` anywhere in
      `sdk/go`.
- [x] Failing-first tests, driven by the golden fixtures: every frame encodes to exact bytes and
      decodes to the expected `ControlFrame`, including a frame carrying both `event` and `method`
      (event wins) and malformed input producing a parse error.
- [x] Adding a second envelope to the spec requires no changes to the emitter's Go templates beyond
      data (demonstrated by a unit test over a synthetic `EnvelopeSpec`, not by shipping one).

## Progress
- 2026-08-03: Started after R-8 settled the hand-written `ControlFrame` and `Envelope` target API;
  first model the frozen classic.v1 grammar, then emit and byte-prove the Go codec.
- 2026-08-03: Completed the validated target-neutral envelope/fixture model, authored classic.v1
  declaration, generated Go codec and comprehensive exact-byte/semantic tests. A sentinel-bearing
  synthetic v2 spec proves the emitter treats every envelope name as data. Rust, clippy, drift, Go,
  race, and website gates pass.

## Notes
- The `Envelope` interface itself and `ControlFrame` / `WireError` are hand-written runtime types
  (R-8); only the codec implementation is generated.
- Correlation ids are minted by the session, not the envelope — the codec treats ids as opaque.
- The imported legacy session's old byte DTO/parser remains isolated until R-9 can switch the
  session and remove that package without creating a temporary root-package import cycle; the new
  runtime has only the generated codec implementation.
