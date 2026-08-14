---
id: R-34
title: Add a spec-owned transport and profile registry
pillar: Spec
status: backlog
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
- [ ] The model validates transport, envelope, catalog, profile, negotiation-token/default, reserved
      signaling, and media-constraint references without encoding SDK implementation policy.
- [ ] Existing `rtvbp.v1`, `rtvbp.demo.v1`, and `rtvbp.webrtc.v1` declarations reproduce current
      names, composition, selection order, and headerless compatibility exactly.
- [ ] The generator emits a deterministic profile manifest, Go/Rust/TypeScript constants, public docs,
      and valid/invalid negotiation vectors from the same declarations.
- [ ] Synthetic second binding/profile tests prove every emitter is data-driven and reject collisions,
      dangling references, and ambiguous defaults.
- [ ] Regeneration changes no frozen catalog/envelope bytes and the complete gate rejects profile drift.
