---
id: R-34
title: Add a spec-owned transport and profile registry
pillar: Spec
status: done
priority: 35
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [spec, generator, sdk-go, sdk-rust, sdk-typescript, website]
note: generate profile names, composition, negotiation, and constraints before a third SDK copies them
---

# Add a spec-owned transport and profile registry

## Goal
Make the declarative facts about transports and profiles another projection of the executable spec,
while leaving procedural network behavior hand-written.

## Acceptance
- [x] The model validates transport, envelope, catalog, profile, negotiation-token/default, reserved
      signaling, and media-constraint references without encoding SDK implementation policy.
- [x] Existing `rtvbp.v1`, `rtvbp.demo.v1`, and `rtvbp.webrtc.v1` declarations reproduce current
      names, composition, selection order, and headerless compatibility exactly.
- [x] The generator emits a deterministic profile manifest, Go/Rust/TypeScript constants, public docs,
      and valid/invalid negotiation vectors from the same declarations.
- [x] Synthetic second binding/profile tests prove every emitter is data-driven and reject collisions,
      dangling references, and ambiguous defaults.
- [x] Regeneration changes no frozen catalog/envelope bytes and the complete gate rejects profile drift.

## Progress

- 2026-08-14: Added a target-neutral profile model with reference, uniqueness, token, preference,
  default/headerless, reserved-signaling, media-carrier, and media-format validation. Failing-first
  model tests cover a complete synthetic binding plus collisions, dangling references, ambiguous
  fallback, namespace misuse, and invalid media references.
- 2026-08-14: Authored the current three-profile registry in its own spec crate and froze the existing
  plain/demo/WebRTC order, headerless compatibility, catalog/envelope composition, one audio channel,
  PCMU WebRTC boundary, L16 SDK boundary, and non-trickle offer method.
- 2026-08-14: All six emitter targets now project registry facts. Go/Rust runtime constants consume
  generated authority, TypeScript executes generated valid/invalid negotiation vectors, and a
  synthetic second profile reaches every emitter without a special case.
