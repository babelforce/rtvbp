---
id: R-16
title: CI drift gate, Taskfile, first release and rtvbp-go deprecation
pillar: Proof
status: in-progress
priority: 17
design: docs/designs/conformance.md
epic: conformance
areas: [conformance, sdk-go]
note: Go v0.1.0-rc.3 adds selectable Pion WebRTC; stable tag and legacy retirement remain
---

# CI drift gate, Taskfile, first release and rtvbp-go deprecation

## Goal
Make the guarantee self-enforcing — generated output cannot drift from the spec without failing the
build — and retire the old repository cleanly now that its consumers have somewhere to go.

## Acceptance
- [x] `task generate` runs every emitter; `task check` runs the full gate locally.
- [x] CI runs the chain in order: `cargo test` → `task generate` → `git diff --exit-code` →
      `go test ./...` → docs build. A deliberately stale generated file fails the build (verified
      once, then reverted).
- [x] Generated output is committed, so `go get` works without running the generator.
- [ ] `sdk/go` is tagged `sdk/go/v0.1.0`.
- [ ] `rtvbp-go` gets a final `v0.41.0` release whose README points at the monorepo and states that
      published versions remain available from the module proxy indefinitely; the repository is then
      archived.
- [ ] The roadmap's Status and Delivered sections are updated to reflect the shipped milestone.

## Progress
- 2026-08-04: Started after the separately maintained `rtvbp-openai` service passed local and Fly
  mic/speaker acceptance with duplex audio, DTMF, speech barge-in and clean termination.
- 2026-08-04: Added root `task generate` and `task check`, made CI execute the same ordered gate,
  and added a root `sdk/go/v*` release workflow. The clean full gate passes. A disposable worktree
  with a committed stale generated docs artifact fails the build and was removed afterward.
- 2026-08-04: Prepared the legacy repository's final deprecation notice on clean local branch
  `r16-final-release` at commit `e00dcb6`; its normal, race, vet and tidy gates pass. The unrelated
  uncommitted change in its `main` worktree was left untouched. Publishing the monorepo/tag, merging
  the OpenAI branch, releasing legacy `v0.41.0`, and archiving remain external release actions.
- 2026-08-04: Published `main` and `sdk/go/v0.1.0-rc.1`. The protocol gate, prerelease workflow and
  Pages deployment all completed successfully; the generated operation reference serves publicly,
  and a clean module resolved the RC through `proxy.golang.org`. The `rtvbp-openai` migration branch
  now pins the public RC without a local `replace`, with normal, race and vet tests green.
- 2026-08-04: Prepared `sdk/go/v0.1.0-rc.2` after the additive WebRTC epic. It retains plain
  WebSocket audio and adds the Pion binding, combined demo selection, full race-tested acceptance,
  and matching public documentation; stable `v0.1.0` remains gated by the legacy retirement work.
- 2026-08-04: RC.2's release workflow exposed that clean Ubuntu runners lacked the PortAudio system
  library now that the existing demo modules are part of `task check`. Declared `portaudio19-dev` in
  both workflows and advanced to RC.3 without moving or deleting the failed RC.2 tag.

## Notes
- Interop (R-12) may need network access to the Go proxy in CI; if that is unavailable, vendor
  `v0.37.2` and record the decision rather than dropping the test.
- R-8 makes `go test ./...` literal by fixing module/workspace paths; do not reintroduce a
  `GOWORK=off` carve-out in CI.
- Do not archive `rtvbp-go` before the `rtvbp-openai` branch from R-15 is merged and deployed.
