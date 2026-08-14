---
id: R-33
title: Capture TypeScript authority and settle JavaScript wire semantics
pillar: Proof
status: in-progress
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
- [ ] The maintained external TypeScript client and browser tests are source-pinned; every behavior is
      inventoried as protocol, deployment, application convenience, or defect without treating it as
      wire authority.
- [ ] Failing-first TypeScript tests consume all frozen payload/envelope fixtures and demonstrate the
      current gaps in field presence/order, discriminator precedence, permissive responses, errors,
      unknown dispatch, and correlation.
- [ ] A design decision covers `int64`, float, open-map, optional/null, and lossless JSON behavior;
      unsafe integers cannot be silently rounded.
- [ ] Browser/Node targets, package format, runtime dependency policy, cancellation, and media ownership
      are explicit enough for the emitter and runtime stories to implement without guessing.
- [ ] The evidence capture is disposable or clearly separated from generator-owned output and adds no
      second source of truth.
