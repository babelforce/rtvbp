---
id: R-4
title: Prove spec byte-equality against the golden fixtures
pillar: Proof
status: in-progress
priority: 4
design: docs/designs/spec-catalog.md
epic: spec-catalog
areas: [spec, conformance]
note: frozen fixtures and the typed catalog are ready; prove complete bidirectional byte equality
---

# Prove spec byte-equality against the golden fixtures

## Goal
Pin the spec to the deployed wire before any emitter exists, so every downstream artifact inherits a
proven-correct model rather than a plausible one.

## Acceptance
- [ ] A failing-first test in the spec crate serializes each canonical example and asserts
      `bytes ==` the corresponding frozen fixture from `conformance/babelforce.v1/golden/`.
- [ ] The test covers envelope frames as well as payloads, via the `classic.v1` reference codec:
      constant `version:"1"`, discrimination order, response without its own id, and `error.data`
      under the key `"any"`.
- [ ] Deserialization is proven in both directions: every fixture parses into its spec type and
      re-serializes identically.
- [ ] Any mismatch is resolved by changing the **spec**, never the fixture; the fixtures stay frozen.
- [ ] A short note in the design doc records every presence/ordering subtlety this story uncovered,
      so the emitters do not have to rediscover them.

## Progress
- 2026-08-03: Started; adding a failing-first bidirectional byte round-trip test over all 29 frozen
  payload, event, and `classic.v1` envelope fixtures.

## Notes
- This is the highest-value story in M1: it is where the byte-identity bet is won or lost, and it
  costs nothing to iterate on because no generated code exists yet.
- Known candidates for trouble: `null` vs. omitted, field order, integer widths, the bare-map
  `session.get` result, and `params` omitted when nil.
