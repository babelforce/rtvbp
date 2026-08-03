---
id: R-18
title: Settle frozen v1 semantic constraints before generation
pillar: Spec
status: in-progress
priority: 5
design: docs/reviews/2026-08-03-soundness-review.md
epic: spec-catalog
areas: [spec, conformance]
note: review follow-up; formalizes termination, errors and reserved names before emitters consume them
---

# Settle frozen v1 semantic constraints before generation

## Goal
Turn the semantic assumptions found by the soundness review into explicit, tested protocol
decisions before generated codecs, role interfaces, and public documentation depend on them.

## Acceptance
- [ ] `session.terminate` remains `handled_by: Application`: deployed voice clients call it and
      deployed application handlers answer it. The reverse application→voice request remains the
      frozen explicit 501 behavior, not an undeclared `Both` extension; catalog, runtime plan,
      scenarios, and docs agree.
- [ ] The `classic.v1` reference contract explicitly decides and tests whether responses containing
      both or neither of `result` and `error` are accepted, based on deployed decoder behavior rather
      than JSON-RPC assumptions.
- [ ] Error validation no longer invents constraints: code `0`, empty messages, and unknown integer
      codes are either accepted or rejected from observed authority, with failing-first tests and a
      documented convention registry for known deployed codes `-1`, `400`, `500`, and `501`.
- [ ] The catalog validator rejects every operation in the reserved `transport.*` namespace; R-13
      remains responsible for publishing that reservation.
- [ ] The spec/catalog design records each decision, and the full existing golden and typed catalog
      suites remain green without modifying frozen fixtures.

## Progress
- 2026-08-03: Repository audit resolved the role contradiction in favor of the frozen deployed
  direction (`Application`); codec and namespace decisions remain to implement.
- 2026-08-03: Started; deriving response/error permissiveness from the deployed Go decoder and
  adding failing-first validation tests for the reserved namespace.

## Notes
- Review: [`docs/reviews/2026-08-03-soundness-review.md`](../reviews/2026-08-03-soundness-review.md),
  especially S3, I2, I3, N4, and the validation half of N5.
- Evidence for the role decision: the v0.40 voice client sends `session.terminate`, application
  handlers answer it, and the voice-side reverse handler deliberately returns 501.
- This story blocks R-6 and R-7; it does not fork `babelforce.v1` or change its wire.
