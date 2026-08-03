---
id: R-4
title: Prove spec byte-equality against the golden fixtures
pillar: Proof
status: done
priority: 4
design: docs/designs/spec-catalog.md
epic: spec-catalog
areas: [spec, conformance]
note: all 29 frozen fixtures now round-trip byte-identically through their concrete spec types
---

# Prove spec byte-equality against the golden fixtures

## Goal
Pin the spec to the deployed wire before any emitter exists, so every downstream artifact inherits a
proven-correct model rather than a plausible one.

## Acceptance
- [x] A failing-first test in the spec crate serializes each fixture-backed canonical example and
      asserts `bytes ==` the corresponding frozen fixture from
      `conformance/babelforce.v1/golden/`.
- [x] The test covers envelope frames as well as payloads, via the `classic.v1` reference codec:
      constant `version:"1"`, discrimination order, response without its own id, and `error.data`
      under the key `"any"`.
- [x] Deserialization is proven in both directions: every fixture parses into its spec type and
      re-serializes identically.
- [x] Any mismatch is resolved by changing the **spec**, never the fixture; the fixtures stay frozen.
- [x] A short note in the design doc records every presence/ordering subtlety this story uncovered,
      so the emitters do not have to rediscover them.

## Progress
- 2026-08-03: Started; adding a failing-first bidirectional byte round-trip test over all 29 frozen
  payload, event, and `classic.v1` envelope fixtures.
- 2026-08-03: Done; the inventory guard owns all 29 existing fixtures, concrete typed round-trips
  preserve every byte, and `classic.v1` decodes/re-encodes all four envelope shapes exactly. The
  only mismatch (integral Go `float64` spelling) was corrected in the spec, never the fixtures.

## Notes
- This is the highest-value story in M1: it is where the byte-identity bet is won or lost, and it
  costs nothing to iterate on because no generated code exists yet.
- Known candidates for trouble: `null` vs. omitted, field order, integer widths, the bare-map
  `session.get` result, and `params` omitted when nil.
- Scope is the complete existing 29-fixture v0.40.0 capture: 20 operation payloads, five deployed-Go
  event payloads, and four `classic.v1` envelopes. The four additive browser/Rust events have typed
  canonical examples but no v0.40.0 fixture; R-17 adds their independently sourced wire authority
  before Go types are emitted.
