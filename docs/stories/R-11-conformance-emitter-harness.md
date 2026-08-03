---
id: R-11
title: Conformance vector emitter and the Go harness
pillar: Proof
status: done
priority: 12
design: docs/designs/conformance.md
epic: conformance
areas: [generator, conformance, sdk-go]
note: generated payload/envelope vectors and typed scenarios now execute through both Go roles
---

# Conformance vector emitter and the Go harness

## Goal
Make cross-SDK agreement mechanical: emit language-neutral test vectors and multi-message scenarios
from the same catalog that produces the SDKs, and consume them from Go through a thin harness that
every future SDK mirrors.

## Acceptance
- [x] `--emit=vectors` writes `conformance/babelforce.v1/`: `payloads/<method>.json` (valid
      byte-exact samples plus invalid samples with an expected error class),
      `envelope/classic.v1/frames.json` (encode and decode cases, including discrimination-order and
      malformed input), and `scenarios/*.json`.
- [x] Scenarios are authored as typed Rust in the spec crate — so they cannot drift from the
      schemas — and serialized by the emitter, using `$name` bindings for generated ids.
- [x] Three scenarios ship: `initialize → updated → dtmf`; termination covering
      application-initiated `call.hangup`, supported voice→application `session.terminate`, and the
      reverse-direction 501; and a `ping` RTT exchange.
- [x] A hand-written Go harness reads the committed vectors, plays the scripted peer for one role
      over the memory transport, and asserts the side under test — for both roles.
- [x] Matching is byte-exact on encode-side checks and structural (after id normalization) for
      messages the SDK originates.
- [x] The harness reads the vectors from the monorepo path; no copies are vendored into the SDK.

## Progress
- 2026-08-03: Started after the generated role and documentation surfaces closed. Auditing catalog
  ownership for typed scenarios, byte-exact payload/envelope vector formats, binding normalization,
  terminal-session boundaries, and the memory-transport harness for both local roles.
- 2026-08-03: Added typed, catalog-validated scenario declarations and the deterministic vectors
  target with payload error classes, all classic-envelope encode/decode fixtures, malformed cases,
  and a structural-precedence case. Generated 14 committed vector files without touching golden
  authority.
- 2026-08-03: Added the hand-written Go harness over the shared monorepo vectors. Exact payload and
  envelope encoding, structural generated-id matching, both local roles, three termination paths,
  event ordering and bidirectional ping all pass over the memory transport; Rust tests/clippy, Go,
  drift checks and the production documentation build pass.

## Notes
- The vectors are generated; the harness is deliberately hand-written and small — generating it
  would cost more than it saves.
- Keep encode-side checks strict; loose matching is how a conformance suite quietly stops catching
  regressions.
