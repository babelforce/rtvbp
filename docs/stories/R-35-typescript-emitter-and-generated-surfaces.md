---
id: R-35
title: Generate the TypeScript catalog, roles, peers, and classic envelope
pillar: Generator
status: done
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
- [x] The emitter implements the R-33 numeric/presence decision and has synthetic tests across schema
      shapes, names, roles, terminality, rejections, validation, and a second envelope/catalog.
- [x] Generated payloads construct and byte-round-trip every frozen fixture; malformed and semantic
      vectors fail with structured errors rather than JavaScript coercion.
- [x] Both roles receive generated handler/adapter APIs, typed operation peers, event emitters, unknown
      hooks, and terminal metadata with the same capabilities as Go and Rust.
- [x] `classic.v1` is generated from `EnvelopeSpec` and preserves every deployed discriminator, null,
      error, omission, ordering, and response-permissiveness quirk.
- [x] Browser-neutral ESM, declarations, package exports, DO-NOT-EDIT ownership, formatting, typecheck,
      tests, and generated-drift checks are part of `task check`.

## Progress

- 2026-08-14: Added a catalog-agnostic TypeScript emitter for ordered payload interfaces, optional
  versus required-nullable presence, safe-number structured validators, method/event constants,
  exact serializers, and strict decoders. Synthetic schema, naming-collision, validation, second
  catalog, and second-envelope tests keep the emitter data-driven.
- 2026-08-14: Generated both-role handlers and adapters, terminal metadata, the frozen per-role
  rejection, typed peers, peer-event handlers, event emitters, and explicit unknown hooks. Generated
  tests execute every operation and event surface in both roles.
- 2026-08-14: Generated `classic.v1` from `EnvelopeSpec` and executed all frozen payload and envelope
  witnesses byte-for-byte, including structural precedence, omission versus null, open error codes,
  the legacy error-data key, null-error normalization, and permissive responses.
- 2026-08-14: Added a single browser/Node ESM package entry, declaration build, browser-only compile
  without Node types, Node package import, dry-run package inspection, and generated ownership/drift
  checks. The release gate also requires npm lockfiles to resolve only through the public default
  registry.
