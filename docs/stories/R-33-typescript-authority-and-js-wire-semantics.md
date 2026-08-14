---
id: R-33
title: Capture TypeScript authority and settle JavaScript wire semantics
pillar: Proof
status: done
priority: 34
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [sdk-typescript, conformance, spec]
note: pin the browser migration evidence and fail closed on JavaScript's wire hazards before generation
---

# Capture TypeScript authority and settle JavaScript wire semantics

## Goal
Turn the existing browser client into source-pinned migration evidence and resolve the numeric,
presence, parsing, and lifecycle questions that could otherwise make generated TypeScript look typed
while silently changing frozen bytes.

## Acceptance
- [x] The maintained external TypeScript client and browser tests are source-pinned; every behavior is
      inventoried as protocol, deployment, application convenience, or defect without treating it as
      wire authority.
- [x] Failing-first TypeScript tests consume all frozen payload/envelope fixtures and demonstrate the
      current gaps in field presence/order, discriminator precedence, permissive responses, errors,
      unknown dispatch, and correlation.
- [x] A design decision covers `int64`, float, open-map, optional/null, and lossless JSON behavior;
      unsafe integers cannot be silently rounded.
- [x] Browser/Node targets, package format, runtime dependency policy, cancellation, and media ownership
      are explicit enough for the emitter and runtime stories to implement without guessing.
- [x] The evidence capture is disposable or clearly separated from generator-owned output and adds no
      second source of truth.

## Progress

- 2026-08-14: Captured the maintained browser consumer by opaque SHA-256 source/test digests, split
  observed behavior into protocol, deployment, application, and defect categories, and retained no
  private repository coordinate in the public tree.
- 2026-08-14: The initial fixture test failed before the wire foundation existed. The implemented
  lossless parser/strict encoder now byte-round-trips all 48 frozen fixtures, rejects unsafe numeric
  conversion and JSON coercions, and runs under Node's TypeScript test runner plus strict `tsc`.
- 2026-08-14: Settled ES2022 ESM targets, dependency policy, number/presence/open-map semantics,
  cancellation, session ownership, and media ownership in the M2 design.
