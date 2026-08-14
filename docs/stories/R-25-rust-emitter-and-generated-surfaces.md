---
id: R-25
title: Rust emitter and generated protocol surfaces
pillar: Generator
status: done
priority: 22
design: docs/designs/rust-sdk.md
epic: rust-sdk
areas: [sdk-rust, generator]
note: generate Rust payloads, roles, envelope codec, and byte proofs from the same spec as Go
---

# Rust emitter and generated protocol surfaces

## Goal
Make Rust a first-class generator target so no catalog, role, validation, or envelope wire logic is
hand-written in the Rust SDK.

## Acceptance
- [x] A failing-first synthetic-catalog test proves the Rust emitter is catalog-agnostic and
      preserves required, optional, required-nullable, ordered, referenced, open-object, numeric,
      array, and structured-validation shapes.
- [x] `rtvbp-spec-gen --emit=rust` emits committed `babelforce.v1` and `demo.v1` payloads, method and
      event identities, role handler traits/adapters, typed peers, event emitters, terminal behavior,
      and exact per-role rejections without recognizing catalog spellings.
- [x] The generated `classic.v1` Rust envelope codec reproduces every frozen encode/decode fixture,
      including field order, structural precedence, required-null error data, permissive legacy
      responses, malformed input behavior, and receive timestamps.
- [x] Generated Rust fixture and role-contract tests prove construction plus decode/re-encode parity;
      every file carries the DO-NOT-EDIT banner and Rust target ownership removes stale generated
      files without touching hand-written files.
- [x] `task generate`, generator `--emit=rust --check`, and CI include the Rust target; a deliberately
      stale generated Rust file fails the drift check.

## Progress
- 2026-08-14: Started from the explicit requirement that this monorepo supersede every prior RTVBP
  implementation and that the Rust SDK reach full Go parity, including WebRTC.
- 2026-08-14: Accepted `docs/designs/rust-sdk.md`; the legacy AI Platform Rust crate is migration
  evidence only, and its hand-written protocol files are excluded from the new SDK.
- 2026-08-14: Added the first-class Rust generator target, committed standalone SDK crate, generated
  payload/validation/role/envelope surfaces, construction and executable role proofs, Go-compatible
  float spelling, and catalog/envelope/stale-output synthetic tests. Rust format, clippy and tests
  are now part of `task check`; the complete spec and Rust R-25 gates pass.

## Notes
- The runtime types referenced by generated roles and codec are introduced as the smallest compiling
  skeleton here; behavioral runtime work belongs to R-26.
