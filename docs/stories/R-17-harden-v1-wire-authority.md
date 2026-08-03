---
id: R-17
title: Complete babelforce.v1 wire authority and fixture variants
pillar: Proof
status: done
priority: 4
design: docs/reviews/2026-08-03-soundness-review.md
epic: conformance
areas: [spec, conformance]
note: 46 source-pinned fixtures now cover every catalog shape and the missing wire variants
---

# Complete babelforce.v1 wire authority and fixture variants

## Goal
Close the wire-authority and coverage gaps found by the 2026-08-03 soundness review before the
generator turns unobserved shapes into public SDK and documentation surfaces.

## Acceptance
- [x] Reproducible, provenance-pinned capture adds golden payload fixtures for the four additive
      browser/Rust events absent from `rtvbp-go v0.40.0`: `output.transcript.delta`,
      `output.transcript.done`, `input.transcript`, and `agent.tool.call`. The existing 29 Go-derived
      fixtures remain byte-identical.
- [x] The capture suite pins the presence/formatting variants the first capture missed: a request
      envelope with `params`, an error without `any` (including deployed error codes), omitted
      optional payload fields, integral and fractional `float64` spellings, and success responses
      with absent and explicit-null results.
- [x] The authority of each `session.terminate` response fixture is explicit: the success `{}` comes
      from the application/demo-server handler, while an application→voice request is pinned to the
      deployed 501 error response rather than treated as a successful reverse-direction operation.
- [x] A reproducible comparison proves the common v0.37.2 and v0.40.0 catalog encodings used by
      fixtures are byte-equivalent, or records any differences and scopes the compatibility claim
      before live interop relies on it.
- [x] Every added fixture has a failing-first typed deserialize → reserialize byte-equality test;
      the inventory test makes an unproved fixture or catalog event fail loudly.
- [x] The golden README distinguishes Go v0.40.0 captures from additive-event authority and records
      struct order, sorted-map order, optional/null behavior, float formatting, and unknown-field
      tolerance.

## Progress
- 2026-08-03: Filed from the soundness review while R-4 made the existing 29-fixture scope explicit;
  no emitted SDK surface may depend on the four additive events until this story is done.
- 2026-08-03: Started; auditing the pinned Rust/browser source and extending the reproducible capture
  before adding any new authority fixture.
- 2026-08-03: Done; 46 fixtures are covered by exact inventories and typed/codec round-trips. The
  pinned v0.37.2 capture matches all 40 common bytes; six exclusions are classified. Variant proof
  fixed explicit-null result preservation and enabled exact fractional float parsing in the spec.

## Notes
- Review: [`docs/reviews/2026-08-03-soundness-review.md`](../reviews/2026-08-03-soundness-review.md),
  especially B1, S2, fixture-coverage findings 1–6, and N7.
- This story blocks R-6. R-4 remains the proof that all 29 existing frozen Go fixtures round-trip
  byte-identically; this story expands the authority set without rewriting it.
