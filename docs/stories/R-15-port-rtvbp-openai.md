---
id: R-15
title: Port rtvbp-openai to the new SDK as the acceptance test
pillar: Proof
status: ready
priority: 16
design: docs/designs/conformance.md
epic: conformance
areas: [conformance]
note: published-version interop passes; ready for the real-service phone-call acceptance proof
---

# Port rtvbp-openai to the new SDK as the acceptance test

## Goal
Prove the new SDK is a drop-in for a real service, not just for tests: `rtvbp-openai` — which today
pins `github.com/babelforce/rtvbp-go v0.37.2` — runs a live call against it.

## Acceptance
- [ ] `rtvbp-openai` builds against `github.com/babelforce/rtvbp/sdk/go` on a branch.
- [ ] The diff touches only import paths, constructor calls, and the generated type/identifier
      renames — nothing structural. If more is needed, the SDK's ergonomics regressed and that is
      recorded as a finding on the `go-sdk` design.
- [ ] A real end-to-end phone call works: audio both ways, barge-in via `audio.buffer.clear`, a
      `dtmf` event handled, and clean termination.
- [ ] The branch is not merged into `rtvbp-openai` until `sdk/go` is tagged (R-16).

## Progress
- (not started)

## Notes
- `rtvbp-openai` exercises a good slice of the protocol: typed request and event handlers,
  `CallHangupRequest`, `AudioBufferClearRequest`, `ApplicationMoveRequest`, the speech-started event,
  and the ping helpers.
- It is also the one consumer that proves the application-role generated interface is pleasant to
  implement.
