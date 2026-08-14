---
id: R-35
title: Generate the TypeScript catalog, roles, peers, and classic envelope
pillar: Generator
status: in-progress
priority: 36
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [generator, sdk-typescript, conformance]
note: emit the complete browser-neutral TypeScript protocol surface from the same spec as Go and Rust
---

# Generate the TypeScript catalog, roles, peers, and classic envelope

## Goal
Create `sdk/typescript` with no hand-written wire types: payloads, validation, role APIs, typed peers,
events, and the frozen envelope all derive from the existing spec.

## Acceptance
- [ ] The emitter implements the R-33 numeric/presence decision and has synthetic tests across schema
      shapes, names, roles, terminality, rejections, validation, and a second envelope/catalog.
- [ ] Generated payloads construct and byte-round-trip every frozen fixture; malformed and semantic
      vectors fail with structured errors rather than JavaScript coercion.
- [ ] Both roles receive generated handler/adapter APIs, typed operation peers, event emitters, unknown
      hooks, and terminal metadata with the same capabilities as Go and Rust.
- [ ] `classic.v1` is generated from `EnvelopeSpec` and preserves every deployed discriminator, null,
      error, omission, ordering, and response-permissiveness quirk.
- [ ] Browser-neutral ESM, declarations, package exports, DO-NOT-EDIT ownership, formatting, typecheck,
      tests, and generated-drift checks are part of `task check`.
