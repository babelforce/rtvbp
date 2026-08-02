---
id: R-11
title: Conformance vector emitter and the Go harness
pillar: Proof
status: backlog
design: docs/designs/conformance.md
epic: conformance
areas: [generator, conformance, sdk-go]
note: blocked on R-10; the mechanism that rolls abstract e2e tests into every SDK
---

# Conformance vector emitter and the Go harness

## Goal
Make cross-SDK agreement mechanical: emit language-neutral test vectors and multi-message scenarios
from the same catalog that produces the SDKs, and consume them from Go through a thin harness that
every future SDK mirrors.

## Acceptance
- [ ] `--emit=vectors` writes `conformance/babelforce.v1/`: `payloads/<method>.json` (valid
      byte-exact samples plus invalid samples with an expected error class),
      `envelope/classic.v1/frames.json` (encode and decode cases, including discrimination-order and
      malformed input), and `scenarios/*.json`.
- [ ] Scenarios are authored as typed Rust in the spec crate — so they cannot drift from the
      schemas — and serialized by the emitter, using `$name` bindings for generated ids.
- [ ] Three scenarios ship: `initialize → updated → dtmf`; an application-initiated hangup covering
      terminal-operation semantics; and a `ping` RTT exchange.
- [ ] A hand-written Go harness reads the committed vectors, plays the scripted peer for one role
      over the memory transport, and asserts the side under test — for both roles.
- [ ] Matching is byte-exact on encode-side checks and structural (after id normalization) for
      messages the SDK originates.
- [ ] The harness reads the vectors from the monorepo path; no copies are vendored into the SDK.

## Progress
- (not started)

## Notes
- The vectors are generated; the harness is deliberately hand-written and small — generating it
  would cost more than it saves.
- Keep encode-side checks strict; loose matching is how a conformance suite quietly stops catching
  regressions.
