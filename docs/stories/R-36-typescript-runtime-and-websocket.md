---
id: R-36
title: Implement the TypeScript session runtime and WebSocket binding
pillar: SDK
status: backlog
priority: 37
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [sdk-typescript, conformance]
note: match Go/Rust session semantics in browsers and Node before adding device-media policy
---

# Implement the TypeScript session runtime and WebSocket binding

## Goal
Add the hand-written semantic runtime, memory transport, and deployed WebSocket binding underneath the
generated TypeScript surfaces without importing browser UI or deployment authentication policy.

## Acceptance
- [ ] Semantic frames, lifecycle, response fast path, serial dispatch, nested requests, timeouts,
      deferred/terminal replies, pending arbitration, middleware, keepalive, and bounded close match the
      executable Go/Rust contracts.
- [ ] Session-owned media buffering has explicit format, ownership, backpressure, clear, timing, and
      cancellation behavior; no unknown request is acknowledged implicitly.
- [ ] Memory conformance executes every generated scenario with either role local and has leak/unhandled
      rejection guards.
- [ ] WebSocket clients run in browsers and Node through an injected platform seam; the Node server
      supports both generated roles, profile negotiation, binary L16 media, Ping/Pong, and flush-on-close.
- [ ] Live TypeScript↔Go and TypeScript↔Rust WebSocket tests cover both role directions, typed control,
      duplex non-silent audio, terminal close, and headerless v1 compatibility.
