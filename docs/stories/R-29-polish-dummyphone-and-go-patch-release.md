---
id: R-29
title: Polish the dummyphone and publish the Go patch release
pillar: SDK
status: in-progress
priority: 26
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [sdk-go, examples, release]
note: make the demo telephony adapter stateful and race-safe, then publish sdk/go/v0.1.1
---

# Polish the dummyphone and publish the Go patch release

## Goal
Make the demo client’s telephony adapter safe and useful under every generated bridge operation,
then publish the accumulated Go SDK changes as a verified patch release.

## Acceptance
- [x] Failing-first tests prove DTMF before registration is harmless, registered sequences carry
      valid timestamps and monotonic sequence numbers, and nil or duplicate handlers are rejected.
- [x] Session variables and recording operations are stateful, concurrency-safe, context-aware, and
      return errors instead of panicking on invalid input or unknown recordings.
- [x] Hangup is exactly once, invokes callbacks outside the state lock, cancels the demo context,
      and `Hangup`/`Move` reject nil requests without panicking.
- [ ] Focused normal and race tests plus the complete repository release gate pass from the exact
      committed release candidate.
- [ ] `sdk/go/v0.1.1` is published, its GitHub workflow succeeds, and a clean external Go module
      resolves the tag without a local replacement.

## Progress
- 2026-08-14: Started from the pre-registration DTMF regression and the request to polish the full
  dummyphone before releasing every changed SDK surface.
- 2026-08-14: Replaced every dummyphone panic stub with locked state, defensive request/context
  errors, unique recording lifecycles, monotonic serialized DTMF, and exactly-once hangup. Focused
  normal, race, and vet checks pass.

## Notes
- R-28 independently owns publication and external verification of `sdk/rust/v0.1.0` from the same
  release candidate.
