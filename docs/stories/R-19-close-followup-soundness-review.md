---
id: R-19
title: Close follow-up soundness review residuals before vectors and public reference
pillar: Proof
status: done
priority: 11
design: docs/reviews/2026-08-03-soundness-review-2.md
epic: conformance
areas: [spec, conformance, generator, website]
note: follow-up review residuals are closed before vectors and public reference output
---

# Close follow-up soundness review residuals before vectors and public reference

## Goal
Resolve the second soundness review's remaining minor findings before generated vectors and public
reference documentation turn them into compatibility commitments.

## Acceptance
- [x] The exact compatibility envelope of `serialize_go_float64` is documented and boundary-tested
      against Go encoding before conformance vectors are emitted.
- [x] A pinned legacy-Rust capture proves whether `output.transcript.done.text` is present on the
      wire; the inventory, spec model, generated Go type and exact-byte fixtures agree with that
      authority.
- [x] The `transport.*` reservation is explicitly settled as operation-only or expanded to events,
      with validator tests for the chosen rule.
- [x] Envelope documentation records the frozen `error: null` normalization exception explicitly.
- [x] The roadmap reflects the current implementation cadence, and the contributor gate is runnable
      after the R-8 module migration and R-16 gate consolidation.
- [x] The full repository gate and any affected source-pinned capture reproductions pass.

## Progress
- 2026-08-03: Started immediately after the R-9 runtime migration; audit the float boundary and
  pinned transcript authority first because both constrain the later conformance vectors.
- 2026-08-03: Captured Go `float64` spellings by exact bit pattern and pinned the supported deployed
  `bytes_per_second` envelope as `+0` or finite `1e-5..=2^53`, with explicit witnesses for notation,
  signed-zero, high-integral, and non-finite behavior outside it.
- 2026-08-03: Proved the pinned v0.33 production bridge emits transcript-done without text while the
  public serializer permits present non-empty and empty strings; expanded the authority to 48
  fixtures and emitted a Go pointer field that preserves all three presence states.
- 2026-08-03: Settled `transport.*` as an operation-method-only reservation, documented the frozen
  `error:null` normalization exception, refreshed the roadmap and runnable contributor gate, and
  closed adversarial follow-up findings with fail-closed schema-hint and repository-wide AST tests.
- 2026-08-03: Rust tests/clippy, manifest and Go drift checks, full Go and race suites, all three
  source-pinned capture reproductions, nested demos, load test, vet, and website build pass.

## Notes
- Scheduled from the second 2026-08-03 soundness review. These are minor findings, but the float and
  transcript items must land before R-11, and the namespace/error prose must land before R-13.
