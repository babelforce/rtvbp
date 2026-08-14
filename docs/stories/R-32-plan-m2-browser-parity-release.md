---
id: R-32
title: Plan M2 browser parity and the next coordinated release
pillar: Integration
status: done
priority: 33
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [release, spec, generator, sdk-typescript, website, conformance]
note: turn the remaining TypeScript/browser supersession gap into a bounded release train
---

# Plan M2 browser parity and the next coordinated release

## Goal
Reconcile the shipped roadmap, public limitations, and the remaining hand-written browser client,
then define a coherent next release with explicit scope, sequencing, compatibility boundaries, and
go/no-go evidence.

## Acceptance
- [x] The plan distinguishes a major project milestone from a breaking protocol major and names the
      intended component release candidates without forcing unrelated version bumps.
- [x] Every documented WebRTC limitation is either assigned to this milestone or explicitly deferred
      to a new binding version with a reason.
- [x] The remaining TypeScript/browser implementation is inspected and its migration risks—not just
      its features—shape the stories and proof obligations.
- [x] The release is decomposed into ordered stories with one ready starting point, mechanical
      cross-language/browser acceptance, external-consumer migration, and publication criteria.
- [x] The contributor roadmap, public Outlook, changelog, and generated board agree on what is next.

## Progress
- 2026-08-14: Started after `protocol/v1.0.0`, Go v0.1.1, and Rust v0.1.0 shipped. The audit found a
  stale public Outlook, five intentionally frozen WebRTC v1 constraints, and one remaining
  hand-written TypeScript/browser RTVBP implementation outside the protocol repository.
- 2026-08-14: Chose M2 browser parity as an additive `protocol/v1.1.0` train, mapped every limitation,
  inspected the real browser client and tests, decomposed R-33…R-38, and made R-33 the ready
  failing-first foundation. The public site typecheck and production build pass with the new Outlook.

## Notes
- Planning must preserve the frozen `babelforce.v1`, `classic.v1`, `rtvbp.v1`, and
  `rtvbp.webrtc.v1` contracts. A limitation does not authorize silently changing an existing wire.
