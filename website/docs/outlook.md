---
sidebar_position: 99
---

# Outlook

RTVBP separates payloads, envelopes, and transports so new bindings do not require a new
call-control protocol.

The protocol snapshot and parity Go/Rust SDKs are released. Both SDKs implement the plain WebSocket
binding and the optional WebRTC-audio plus WebSocket-control binding.

## Next milestone: browser parity

The next milestone replaces the remaining hand-written browser wire with a spec-generated
TypeScript SDK. It will add:

- generated payloads, validation, roles, typed peers, events, and `classic.v1` envelope code;
- a browser/Node session runtime and WebSocket binding with Go/Rust lifecycle parity;
- browser microphone/speaker adapters for `rtvbp.v1` and the existing `rtvbp.webrtc.v1` profile;
- spec-owned profile metadata so SDK constants, negotiation vectors, and this documentation cannot
  drift independently;
- three-language conformance, real-browser media tests, and migration of the maintained browser
  consumer before publication.

This is an additive protocol milestone. It does not rename or modify the deployed v1 profiles.

## Deliberately later

The current WebRTC limitations remain part of the frozen `webrtcws.v1` contract: one audio channel,
PCMU on the wire, L16/8 kHz/mono/20 ms at the SDK boundary, non-trickle initial negotiation, no ICE
restart or renegotiation, and no transport-owned packet-loss concealment.

Later work includes:

- A coexisting `webrtcws.v2` for Opus/higher-rate audio, trickle ICE, restart, renegotiation, and
  multiple media streams where measured deployment evidence warrants the complexity.
- QUIC and SIP bindings using the same semantic session runtime model.
- Additional versioned catalogs and negotiated profiles, without changing frozen `rtvbp.v1`
  behavior.

New wire behavior is introduced through a new catalog, envelope, or profile. The deployed
`babelforce.v1` catalog and `classic.v1` envelope remain frozen for compatibility.
