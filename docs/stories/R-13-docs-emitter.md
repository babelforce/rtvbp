---
id: R-13
title: Docs emitter — generated protocol reference in the Docusaurus site
pillar: Generator
status: backlog
priority: 14
design: docs/designs/docs-gen.md
epic: docs-gen
areas: [generator, website]
note: blocked on R-5; reference docs become a projection of the catalog
---

# Docs emitter — generated protocol reference in the Docusaurus site

## Goal
Replace hand-written reference prose with a projection of the catalog, so the published
documentation cannot describe a protocol the SDKs do not implement — and give integrators the
per-role view that is the most useful thing to read.

## Acceptance
- [ ] `--emit=docs` writes `website/docs/reference/babelforce.v1/` containing a page per operation
      (params and result field tables with types, presence and descriptions, a JSON example, and a
      direction badge), a page per event, `roles/application` and `roles/voice`
      ("must implement · may call · emits · receives"), and `envelopes/classic-v1`.
- [ ] Field tables, prose and examples all derive from the same schemas, doc comments and canonical
      examples that produce the SDK types and the conformance vectors.
- [ ] A generated `_category_.json` / sidebar fragment per catalog means a second catalog version
      gets its own tree with no hand-edits to `sidebars.ts`.
- [ ] Every generated page carries a DO-NOT-EDIT banner.
- [ ] Two hand-written pages are added and linked: the **WebSocket transport binding** (framing,
      auth, subprotocol) and **profiles & negotiation** (a profile is transport × envelope × catalog;
      absence of a subprotocol means `rtvbp.v1`).
- [ ] The public transport/profile prose reserves the `transport.*` method namespace across every
      catalog and envelope, and the catalog validator rejects operations in that namespace.
- [ ] The Docusaurus site builds; superseded pages under `website/docs/protov1/` are removed or
      reduced to narrative that links into the generated reference.
- [ ] Regenerating produces no diff.

## Progress
- (not started)

## Notes
- The existing prose is already wrong in at least one place — it states the application side sends no
  events, while `audio.speech.started` and the transcript events come from exactly there. Generation
  is the fix.
- Watch MDX escaping of JSON examples and generated tables; the docs build in CI is the check.
