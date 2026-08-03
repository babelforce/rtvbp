---
id: R-13
title: Docs emitter — generated protocol reference in the Docusaurus site
pillar: Generator
status: done
priority: 14
design: docs/designs/docs-gen.md
epic: docs-gen
areas: [generator, website]
note: generated catalog and envelope reference now replaces the stale hand-written v1 prose
---

# Docs emitter — generated protocol reference in the Docusaurus site

## Goal
Replace hand-written reference prose with a projection of the catalog, so the published
documentation cannot describe a protocol the SDKs do not implement — and give integrators the
per-role view that is the most useful thing to read.

## Acceptance
- [x] `--emit=docs` writes `website/docs/reference/babelforce.v1/` containing a page per operation
      (params and result field tables with types, presence and descriptions, a JSON example, and a
      direction badge), a page per event, `roles/application` and `roles/voice`
      ("must implement · may call · emits · receives"), and `envelopes/classic-v1`.
- [x] Field tables, prose and examples all derive from the same schemas, doc comments and canonical
      examples that produce the SDK types and the conformance vectors.
- [x] The generated `classic.v1` page lists the spec's conventional error codes while stating that
      the non-zero integer space is open, and records response both/neither permissiveness.
- [x] A generated `_category_.json` / sidebar fragment per catalog means a second catalog version
      gets its own tree with no hand-edits to `sidebars.ts`.
- [x] Every generated page carries a DO-NOT-EDIT banner.
- [x] Two hand-written pages are added and linked: the **WebSocket transport binding** (framing,
      auth, subprotocol) and **profiles & negotiation** (a profile is transport × envelope × catalog;
      absence of a subprotocol means `rtvbp.v1`).
- [x] The public transport/profile prose reserves the `transport.*` method namespace across every
      catalog and envelope, and the catalog validator rejects operations in that namespace.
- [x] The Docusaurus site builds; superseded pages under `website/docs/protov1/` are removed or
      reduced to narrative that links into the generated reference.
- [x] Regenerating produces no diff.

## Progress
- 2026-08-03: Started after generated Go role glue closed. Auditing the Docusaurus tree, generator
  target ownership, schema projection, envelope semantics and narrative transport/profile boundary
  before replacing the stale hand-written v1 reference.
- 2026-08-03: Added the deterministic docs target with synthetic second-catalog and MDX-escaping
  proofs, stale-file ownership checks, per-operation/event/role pages, recursive shared-type tables,
  structured constraints, canonical examples, and a spec-driven classic.v1 envelope reference.
- 2026-08-03: Replaced the obsolete `protov1` prose with an accurate introduction plus hand-written
  WebSocket binding and profile-negotiation guides. Rust tests/clippy, all generator drift checks,
  the Go suite, and the Docusaurus production build pass with all 22 generated pages bannered.

## Notes
- The existing prose is already wrong in at least one place — it states the application side sends no
  events, while `audio.speech.started` and the transcript events come from exactly there. Generation
  is the fix.
- Watch MDX escaping of JSON examples and generated tables; the docs build in CI is the check.
