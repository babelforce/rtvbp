---
id: R-16
title: CI drift gate, Taskfile, first release and rtvbp-go deprecation
pillar: Proof
status: backlog
priority: 17
design: docs/designs/conformance.md
epic: conformance
areas: [conformance, sdk-go]
note: blocked on R-13 and R-15; makes drift unmergeable and closes out the old repo
---

# CI drift gate, Taskfile, first release and rtvbp-go deprecation

## Goal
Make the guarantee self-enforcing — generated output cannot drift from the spec without failing the
build — and retire the old repository cleanly now that its consumers have somewhere to go.

## Acceptance
- [ ] `task generate` runs every emitter; `task check` runs the full gate locally.
- [ ] CI runs the chain in order: `cargo test` → `task generate` → `git diff --exit-code` →
      `go test ./...` → docs build. A deliberately stale generated file fails the build (verified
      once, then reverted).
- [ ] Generated output is committed, so `go get` works without running the generator.
- [ ] `sdk/go` is tagged `sdk/go/v0.1.0`.
- [ ] `rtvbp-go` gets a final `v0.41.0` release whose README points at the monorepo and states that
      published versions remain available from the module proxy indefinitely; the repository is then
      archived.
- [ ] The roadmap's Status and Delivered sections are updated to reflect the shipped milestone.

## Progress
- (not started)

## Notes
- Interop (R-12) may need network access to the Go proxy in CI; if that is unavailable, vendor
  `v0.37.2` and record the decision rather than dropping the test.
- R-8 makes `go test ./...` literal by fixing module/workspace paths; do not reintroduce a
  `GOWORK=off` carve-out in CI.
- Do not archive `rtvbp-go` before the `rtvbp-openai` branch from R-15 is merged and deployed.
