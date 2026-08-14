---
id: R-38
title: Prove TypeScript parity, migrate the browser consumer, and release M2
pillar: Proof
status: in-progress
priority: 39
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [release, sdk-typescript, sdk-go, sdk-rust, conformance, website]
note: publish only after three-language proof and removal of the maintained hand-written browser wire
---

# Prove TypeScript parity, migrate the browser consumer, and release M2

## Goal
Turn the generated TypeScript SDK into the only maintained browser implementation, prove the
three-language matrix, and publish the additive M2 release train from immutable tags.

## Acceptance
- [x] TypeScript consumes every generated vector/scenario, and the bounded live matrix proves both roles
      over WebSocket plus browser WebRTC media against Go and Rust.
- [ ] The maintained browser consumer replaces its hand-written envelope/payload/client code with the published SDK;
      its browser voice unit, headless echo, and deployed microphone/speaker acceptance remain green.
- [x] Public docs include TypeScript/browser quickstarts, generated profile/reference content, migration,
      auth boundaries, limitations, and runnable WebSocket/WebRTC examples without duplicated wire facts.
- [ ] The npm name is reserved, a clean external project installs the exact package, and the release
      workflow publishes tarball, manifest, checksums, and npm/GitHub provenance from the immutable tag.
- [ ] A release review confirms component diffs and cuts only earned tags—candidate
      `protocol/v1.1.0`, Go/Rust v0.2.0, and TypeScript v0.1.0—then downloads, reproduces, and verifies
      every public artifact.
- [ ] R-16 is complete before the release is described as superseding all maintained RTVBP SDKs.

## Progress

- 2026-08-14: The generated TypeScript harness consumes every payload, envelope, invalid, negotiation,
  and scenario vector. Node sessions prove both roles against Go and Rust; real Chrome proves
  non-silent WebSocket and native WebRTC media against both servers, cancellation, permission
  failure, and resource cleanup. A bounded actual-device microphone/speaker smoke also passes.
- 2026-08-14: Added the public browser/Node quickstart, full generated handler example, WebSocket and
  WebRTC lifecycle/auth/CSP/ICE guidance, migration mapping, discoverability, current limitations,
  and a public session transport diagnostic seam for native WebRTC statistics.
- 2026-08-14: Extended the shared release builder and immutable-tag workflow for TypeScript v0.1.0:
  the exact npm tarball, component notes, manifest, checksums, clean external installation, npm
  provenance, and GitHub attestation are enforced. The name remains unpublished and local npm
  credentials are absent, so reservation/publication is still pending.
- 2026-08-14: Removed scheduler timing from the real-browser barge-in proof: the Go and Rust peers now
  wait for a typed readiness ping after Chrome reports at least 100 ms of queued playback, then issue
  the generated clear request. The complete four-case browser matrix passes three consecutive runs.
