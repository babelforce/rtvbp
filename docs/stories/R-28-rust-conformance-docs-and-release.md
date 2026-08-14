---
id: R-28
title: Rust conformance, documentation, and first release
pillar: Proof
status: in-progress
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
- [ ] The crate packages from committed generated output, a clean external project resolves the
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
- 2026-08-14: `cargo package` verifies from committed generated output, Rust 1.88 checks every
  target, and a clean temporary repository tagged `sdk/rust/v0.1.0` resolves from a separate Cargo
  project. The real GitHub tag remains deliberately uncreated until this work is committed.
- Remaining: create and verify the actual `sdk/rust/v0.1.0` tag after these changes are committed.

## Notes
- Earlier repositories remain named only as frozen compatibility authorities or migration sources;
  this monorepo is the sole implementation source of truth.
