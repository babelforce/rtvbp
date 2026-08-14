---
id: R-28
title: Rust conformance, documentation, and first release
pillar: Proof
status: done
priority: 25
design: docs/designs/rust-sdk.md
epic: rust-sdk
areas: [sdk-rust, conformance, website]
note: prove both-role Go parity, publish the Rust integration path, and gate all generated drift
---

# Rust conformance, documentation, and first release

## Goal
Turn Rust parity into a mechanical repository guarantee and publish a complete integration path from
the same authoritative monorepo.

## Acceptance
- [x] A thin Rust harness consumes every generated payload/envelope vector and typed scenario through
      both roles; no fixture or scenario is restated as a hand-written Rust case.
- [x] Live WebSocket interop succeeds in both role directions against published `rtvbp-go v0.37.2`,
      including negotiated audio, DTMF, application ping, terminal close, and headerless fallback.
- [x] Rust quickstarts and a selectable WebSocket/WebRTC demo compile and run without embedded
      credentials; published docs cover role setup, generated APIs, audio, auth, ICE/TURN, shutdown,
      migration from the ancestor crate, and current binding limits.
- [x] `task check` and CI run Rust format, clippy with warnings denied, all-target tests, generator
      drift, Go/Rust cross-language interop, Go tests, and docs in one matching ordered chain.
- [x] The crate packages from committed generated output, a clean external project resolves the
      tagged `sdk/rust/v0.1.0`, the roadmap records Rust parity, and no current documentation directs
      integrators to an earlier RTVBP repository as an implementation source.

## Progress
- 2026-08-14: Every generated `babelforce.v1` and `demo.v1` vector and scenario passes through the
  thin both-role Rust harness, and a negotiated `rtvbp.demo.v1` live exchange proves runtime profile
  selection remains catalog-agnostic.
  Published v0.37.2 WebSocket interop passes in both directions, including its terminal-close queue
  quirk; current Go/Rust WebRTC interop remains in the same all-target suite.
- 2026-08-14: Added the compile-tested quickstart and runnable selectable demo, public integration,
  auth, ICE/TURN, shutdown, migration and limits docs, package exclusions for repository-only tests,
  and a Rust tag workflow that runs the unified gate, packages, and resolves the tag externally.
- 2026-08-14: The implementation and release-preparation changes are committed. A clean worktree at
  the candidate HEAD passed `task check`; `cargo package` verified all ten committed generated Rust
  artifacts, and a disposable repository tagged `sdk/rust/v0.1.0` resolved from a separate Cargo
  project.
- 2026-08-14: The first clean GitHub runner exposed that a five-second protocol timeout also covered
  the published Go helper's cold dependency download and compilation. Process startup is now
  independently bounded at 60 seconds while every post-connect protocol assertion remains at five.
- 2026-08-14: The first Rust tag run then reproduced the same cold-start boundary in the current-Go
  WebRTC test: its helper finished compiling after the ten-second startup timeout had torn down the
  Rust listener. Both current-Go directions now use the separate 60-second startup bound, keep their
  tighter post-connect assertions, and reap the helper process on failure. A forced-empty Go module
  and build cache reproduces the former 11.6-second startup and now passes both directions.
- 2026-08-14: Tag-job reruns require repository admin rights, so the Rust release workflow now has a
  narrow recovery dispatch: it validates an existing immutable `sdk/rust/v*` tag, gates and packages
  current `main`, resolves the requested tag from a clean external Cargo project, and publishes that
  tag without deleting or moving it.
- 2026-08-14: Published immutable tag `sdk/rust/v0.1.0` at `ee73c2f3`. The administrator recovery
  workflow passed the complete current-main gate, packaged `rtvbp v0.1.0`, resolved the actual tag
  from a clean Cargo project, and created the public GitHub release. A second local clean consumer
  independently resolved and checked the same tag.

## Notes
- Earlier repositories remain named only as frozen compatibility authorities or migration sources;
  this monorepo is the sole implementation source of truth.
