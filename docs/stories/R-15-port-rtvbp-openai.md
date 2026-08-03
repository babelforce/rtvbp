---
id: R-15
title: Port rtvbp-openai to the new SDK as the acceptance test
pillar: Proof
status: in-progress
priority: 16
design: docs/designs/conformance.md
epic: conformance
areas: [conformance]
note: migrated branch passes local duplex speaker, DTMF and termination; Fly deploy and barge-in remain
---

# Port rtvbp-openai to the new SDK as the acceptance test

## Goal
Prove the new SDK is a drop-in for a real service, not just for tests: `rtvbp-openai` — which today
pins `github.com/babelforce/rtvbp-go v0.37.2` — runs a live call against it.

## Acceptance
- [x] `rtvbp-openai` builds against `github.com/babelforce/rtvbp/sdk/go` on a branch.
- [x] The diff touches only import paths, constructor calls, and the generated type/identifier
      renames — nothing structural. If more is needed, the SDK's ergonomics regressed and that is
      recorded as a finding on the `go-sdk` design.
- [ ] A real end-to-end phone call works: audio both ways, barge-in via `audio.buffer.clear`, a
      `dtmf` event handled, and clean termination.
- [ ] The branch is not merged into `rtvbp-openai` until `sdk/go` is tagged (R-16).

## Progress
- 2026-08-04: Started after published-version interop and the second-catalog proof closed. Created
  the unmerged `r15-new-sdk-port` branch in the clean `rtvbp-openai` repository and audited the
  module/import, generated-name, explicit audio negotiation and terminal-close migration surface.
- 2026-08-04: The port passes `go test ./...`, `go vet ./...`, and a real server-start smoke test.
  Restored the bridge's thin `NewPingHandler` convenience after the acceptance diff exposed its
  absence, recorded that finding in the Go SDK design, and kept the service structure unchanged.
  The OpenAI credential is present; a routed phone call is still needed to verify duplex audio,
  barge-in, DTMF, and clean termination together.
- 2026-08-04: A bounded local CLI call exposed the retired OpenAI Realtime Beta API in the demo's
  third-party client. Migrated `rtvbp-openai` itself to the GA WebSocket session/audio shape and
  `gpt-realtime-2.1` on commit `a3837ca`, with failing-first schema and output-audio tests. The live
  rerun sent 319,488 microphone bytes, received and played 98,560 agent audio bytes, spoke the DTMF
  result (confirmed by the listener), and terminated cleanly. Fly deployment still awaits local CLI
  authorization; the final barge-in interaction remains to close this story.

## Notes
- `rtvbp-openai` exercises a good slice of the protocol: typed request and event handlers,
  `CallHangupRequest`, `AudioBufferClearRequest`, `ApplicationMoveRequest`, the speech-started event,
  and the ping helpers.
- It is also the one consumer that proves the application-role generated interface is pleasant to
  implement.
