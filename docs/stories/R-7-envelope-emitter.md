---
id: R-7
title: EnvelopeSpec and the generated Go classic.v1 codec
pillar: Generator
status: backlog
priority: 8
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: blocked on R-6, R-8 and R-18; the generated codec implements settled runtime interfaces
---

# EnvelopeSpec and the generated Go classic.v1 codec

## Goal
Make the envelope a described, generated artifact rather than hand-written per language — so a
second envelope costs a spec entry, and the `classic.v1` quirks are stated once instead of being
re-derived by every SDK author.

## Acceptance
- [ ] `EnvelopeSpec` describes `classic.v1` completely: the constant `version:"1"`, the three frame
      kinds and their field names, structural discrimination in the order **event → method →
      response**, responses correlating via `response` with no id of their own, `params` omitted when
      nil, and the `error.data` → `"any"` key override.
- [ ] `--emit=go` writes `sdk/go/envelope/v1classic/` implementing the runtime's `Envelope`
      interface (`Name`, `Encode`, `Decode`) entirely from that description.
- [ ] The codec is generated — no hand-written encode/decode logic for `classic.v1` anywhere in
      `sdk/go`.
- [ ] Failing-first tests, driven by the golden fixtures: every frame encodes to exact bytes and
      decodes to the expected `ControlFrame`, including a frame carrying both `event` and `method`
      (event wins) and malformed input producing a parse error.
- [ ] Adding a second envelope to the spec requires no changes to the emitter's Go templates beyond
      data (demonstrated by a unit test over a synthetic `EnvelopeSpec`, not by shipping one).

## Progress
- (not started)

## Notes
- The `Envelope` interface itself and `ControlFrame` / `WireError` are hand-written runtime types
  (R-8); only the codec implementation is generated.
- Correlation ids are minted by the session, not the envelope — the codec treats ids as opaque.
