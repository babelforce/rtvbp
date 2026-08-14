---
id: R-27
title: Rust WebRTC plus WebSocket binding
pillar: SDK
status: done
priority: 24
design: docs/designs/rust-sdk.md
epic: rust-sdk
areas: [sdk-rust, conformance, webrtc]
note: implement rtvbp.webrtc.v1 with PCMU duplex media and Go cross-language acceptance
---

# Rust WebRTC plus WebSocket binding

## Goal
Implement the same `webrtcws.v1` binding as Go so Rust can serve or consume authenticated RTVBP
control over WebSocket with timed duplex WebRTC audio.

## Acceptance
- [x] Failing-first signaling tests prove bounded non-trickle `transport.webrtc.offer` correlation
      through the supplied generated envelope and prove signaling never reaches catalog dispatch.
- [x] Stable `webrtc-rs` negotiates exactly one send/receive PCMU/8000/1 transceiver, supports
      caller-supplied ICE servers, and exposes only L16/8000/16-bit/mono/20 ms at the SDK boundary.
- [x] Byte vectors prove little-endian L16/G.711 mu-law conversion; media tests prove one 160-byte
      PCMU sample per outbound frame and RTP-derived PTS on decoded inbound frames.
- [x] Rust client/server composition selects `rtvbp.webrtc.v1` without changing the default
      `rtvbp.v1` binding, and authentication completes before WebSocket upgrade and SDP/ICE work.
- [x] Leak-clean loopback tests prove typed control, terminal flush, non-silent duplex RTP media,
      PCMU selection, cancellation, negotiation timeout, remote control close, peer failure, server
      shutdown, duplicate binding errors, unsupported formats, and idempotent close.
- [x] Cross-language tests pass in both directions: Rust client to Go combined server and Go client
      to Rust combined server, and fail if audio is accidentally carried by WebSocket binary frames.

## Progress
- 2026-08-14: Completed stable `webrtc` 0.14 offer/answer composition, PCMU-only transceiver and
  L16 conversion, timed RTP media, cancellation-safe partial construction, negotiation bounds and
  timeout, profile preservation, and deterministic remote/local close.
- 2026-08-14: Current Go and Rust now pass typed control, terminal flush, and non-silent duplex RTP
  in both client/server directions; a retained base-WebSocket probe proves media never falls back to
  binary WebSocket frames.

## Notes
- Initial implementation pins stable `webrtc` 0.14 with Tokio; alpha releases are not accepted as a
  shortcut to parity.
