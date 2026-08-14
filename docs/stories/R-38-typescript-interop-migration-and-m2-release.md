---
id: R-38
title: Prove TypeScript parity, migrate the browser consumer, and release M2
pillar: Proof
status: backlog
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
- [ ] TypeScript consumes every generated vector/scenario, and the bounded live matrix proves both roles
      over WebSocket plus browser WebRTC media against Go and Rust.
- [ ] The maintained browser consumer replaces its hand-written envelope/payload/client code with the published SDK;
      its browser voice unit, headless echo, and deployed microphone/speaker acceptance remain green.
- [ ] Public docs include TypeScript/browser quickstarts, generated profile/reference content, migration,
      auth boundaries, limitations, and runnable WebSocket/WebRTC examples without duplicated wire facts.
- [ ] The npm name is reserved, a clean external project installs the exact package, and the release
      workflow publishes tarball, manifest, checksums, and npm/GitHub provenance from the immutable tag.
- [ ] A release review confirms component diffs and cuts only earned tags—candidate
      `protocol/v1.1.0`, Go/Rust v0.2.0, and TypeScript v0.1.0—then downloads, reproduces, and verifies
      every public artifact.
- [ ] R-16 is complete before the release is described as superseding all maintained RTVBP SDKs.
