---
id: R-22
title: Pion WebRTC plus WebSocket transport core
pillar: SDK
status: done
priority: 19
design: docs/designs/webrtc.md
epic: webrtc
areas: [sdk-go]
note: Pion signaling and timed PCMU-to-L16 media are implemented and race-tested
---

# Pion WebRTC plus WebSocket transport core

## Goal
Implement the composite Go transport: classic RTVBP control stays on the existing semantic
WebSocket channel while Pion carries one timed bidirectional audio stream.

## Acceptance
- [x] A failing-first signaling test proves an offer and answer are encoded with the supplied
      envelope as a correlated `transport.webrtc.offer` exchange and never reach catalog dispatch.
- [x] `transport/webrtcws` uses Pion v4, negotiates exactly one send/receive PCMU/8000/1 audio
      transceiver, and supports configurable ICE servers with deterministic host-only test defaults.
- [x] Focused vectors prove signed 16-bit little-endian L16 to G.711 mu-law conversion and its
      inverse, including saturation and representative positive/negative samples.
- [x] The media channel accepts only L16/8000/16-bit/mono/20 ms, writes one PCMU RTP media sample per
      session frame, and reads decoded L16 frames with RTP-derived `PTS` and `Timed=true`.
- [x] Context cancellation, malformed/oversized signaling, unsupported media, peer-connection
      failure, duplicate open/accept, and idempotent close have deterministic tested errors.
- [x] No catalog, generated artifact, envelope implementation, or session runtime behavior changes.

## Progress
- 2026-08-04: Started from the new WebRTC epic. The accepted design keeps classic control on the
  existing WebSocket transport, uses Pion v4 with PCMU on RTP, and converts at the transport
  boundary to the frozen v1 L16 byte format.
- 2026-08-04: Implemented envelope-encoded non-trickle offer/answer signaling, a Pion v4 PCMU-only
  peer, G.711 conversion, timed RTP reads, strict media validation, bounded signaling, deterministic
  failure/close behavior, and focused race-clean tests without touching catalog or generated code.

## Notes
- This is a hand-written transport. Do not add `transport.webrtc.*` to a catalog; the namespace is
  reserved precisely so the supplied envelope can carry binding-private negotiation.
- Use non-trickle ICE for `webrtcws.v1`; renegotiation belongs to a later binding version.
