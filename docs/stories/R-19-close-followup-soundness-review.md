---
id: R-19
title: Close follow-up soundness review residuals before vectors and public reference
pillar: Proof
status: backlog
priority: 11
design: docs/reviews/2026-08-03-soundness-review-2.md
epic: conformance
areas: [spec, conformance, generator, website]
note: take after R-9 runtime migration and before R-11/R-13 compatibility outputs
---

# Close follow-up soundness review residuals before vectors and public reference

## Goal
Resolve the second soundness review's remaining minor findings before generated vectors and public
reference documentation turn them into compatibility commitments.

## Acceptance
- [ ] The exact compatibility envelope of `serialize_go_float64` is documented and boundary-tested
      against Go encoding before conformance vectors are emitted.
- [ ] A pinned legacy-Rust capture proves whether `output.transcript.done.text` is present on the
      wire; the inventory, spec model, generated Go type and exact-byte fixtures agree with that
      authority.
- [ ] The `transport.*` reservation is explicitly settled as operation-only or expanded to events,
      with validator tests for the chosen rule.
- [ ] Envelope documentation records the frozen `error: null` normalization exception explicitly.
- [ ] The roadmap reflects the current implementation cadence, and the contributor gate is runnable
      after the R-8 module migration and R-16 gate consolidation.
- [ ] The full repository gate and any affected source-pinned capture reproductions pass.

## Progress
- (not started)

## Notes
- Scheduled from the second 2026-08-03 soundness review. These are minor findings, but the float and
  transcript items must land before R-11, and the namespace/error prose must land before R-13.
