---
id: R-26
title: Rust session, audio, WebSocket transport, and v1 bridge
pillar: SDK
status: done
priority: 23
design: docs/designs/rust-sdk.md
epic: rust-sdk
areas: [sdk-rust, conformance]
note: implement Go-parity runtime semantics and the preserved rtvbp.v1 binding
---

# Rust session, audio, WebSocket transport, and v1 bridge

## Goal
Implement the hand-written Rust runtime and classic WebSocket binding so a generated Rust peer can
replace either side of the Go SDK without lifecycle, dispatch, audio, or failure differences.

## Acceptance
- [x] Failing-first parity tests prove response fast-path plus serial request/event dispatch, nested
      requests, request timeouts, exactly-once pending arbitration, deferred response handles,
      terminal-response flush, middleware, unknown hooks, and connecting/active/closing/closed/failed
      lifecycle behavior.
- [x] Session-owned audio proves immutable negotiated format, exact PTime outbound chunking, inbound
      concatenation, clear-read-buffer safety, timed-frame observation, duplicate binding rejection,
      and orderly versus failed media close.
- [x] Memory transport proves drain-safe control, optional media, cancellation, duplicate open/accept,
      backpressure isolation, and idempotent close without leaked Tokio tasks.
- [x] WebSocket client/server prove authentication-before-upgrade, `rtvbp.v1` negotiation and
      headerless fallback, text control plus optional binary audio, protocol Ping/Pong keepalive,
      single-writer ordering, drain-before-close, remote close, and active server shutdown.
- [x] The hand-written `babelforcev1` bridge implements audio codec selection/binding, initialization,
      ping timing, telephony callbacks, terminal policy, and audio observation entirely on generated
      types and role APIs.

## Progress
- 2026-08-14: Started after R-25 supplied a compiling generated crate and executable catalog/role/
  envelope contracts. The Go runtime and tests remain the behavioral authority for this port.
- 2026-08-14: Completed the Tokio session/audio runtime, drain-safe memory and WebSocket bindings,
  and the generated-role-only v1 voice bridge. The parity suite covers nested and deferred requests,
  ordered dispatch, timeout/arbitration, lifecycle/keepalive, timed duplex media, headerless/auth
  WebSocket behavior, callbacks, observation, and terminal flush.

## Notes
- Algorithms from `private-source.invalid/crates/rtvbp` may be adapted only when the parity tests prove
  their behavior; its hand-written wire types must not enter this repository.
