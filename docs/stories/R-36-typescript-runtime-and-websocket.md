---
id: R-36
title: Implement the TypeScript session runtime and WebSocket binding
pillar: SDK
status: done
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
- [x] Semantic frames, lifecycle, response fast path, serial dispatch, nested requests, timeouts,
      deferred/terminal replies, pending arbitration, middleware, keepalive, and bounded close match the
      executable Go/Rust contracts.
- [x] Session-owned media buffering has explicit format, ownership, backpressure, clear, timing, and
      cancellation behavior; no unknown request is acknowledged implicitly.
- [x] Memory conformance executes every generated scenario with either role local and has leak/unhandled
      rejection guards.
- [x] WebSocket clients run in browsers and Node through an injected platform seam; the Node server
      supports both generated roles, profile negotiation, binary L16 media, Ping/Pong, and flush-on-close.
- [x] Live TypeScript↔Go and TypeScript↔Rust WebSocket tests cover both role directions, typed control,
      duplex non-silent audio, terminal close, and headerless v1 compatibility.

## Progress

- 2026-08-14: Added the browser-neutral supervised session, generated-role handler, bounded memory
  transport and session-owned timed L16 audio stream. Runtime tests cover response fast paths, serial
  dispatch, nested requests, middleware, timeout arbitration, deferred/terminal replies, unknown
  requests, framing, clearing, cancellation and orderly close.
- 2026-08-14: Added injected WebSocket transport seams plus separate browser and Node connectors. The
  Node server negotiates generated profiles, supports headerless v1, both local roles, native
  Ping/Pong and drain-before-close; browser-only compilation has no Node types or imports.
- 2026-08-14: All generated scenarios execute with either role local. Live headerless WebSocket tests
  prove TypeScript against current Go and Rust in both client/server directions with generated typed
  ping/termination and non-silent duplex L16 audio. The package and public-tree gates pass with every
  npm lockfile resolution pinned to the public default registry.
