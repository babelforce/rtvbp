---
sidebar_position: 99
sidebar_label: Roadmap and limits
title: Roadmap and current limits
description: What RTVBP supports today, what remains deliberately frozen, and what may become a new negotiated binding.
---

# Roadmap and current limits

RTVBP separates payloads, envelopes, and transports so new bindings do not require a new
call-control protocol.

The protocol snapshot and parity Go, Rust, and TypeScript SDKs are released. The TypeScript package
adds the same generated control surface for Node and browsers, including plain WebSocket audio and
native browser WebRTC media. The [interactive protocol lab](/try) runs that published SDK directly
in the docs.

## Available today

The current implementation includes:

- generated payloads, validation, roles, typed peers, events, and `classic.v1` envelope code in all
  three SDKs;
- browser/Node, Go, and Rust session runtimes with WebSocket lifecycle parity;
- browser microphone/speaker adapters for `rtvbp.v1` and the existing `rtvbp.webrtc.v1` profile;
- spec-owned profile metadata so SDK constants, negotiation vectors, and this documentation cannot
  drift independently;
- three-language conformance and real-browser non-silent media tests against both Go and Rust.

Browser parity was additive. It did not rename or modify either deployed v1 profile, and the npm,
Go, Rust, and protocol artifacts are published from independently versioned immutable tags.

## Frozen v1 boundary

The current WebRTC limitations remain part of the frozen `webrtcws.v1` contract: one audio channel,
PCMU on the wire, L16/8 kHz/mono/20 ms at the SDK boundary, non-trickle initial negotiation, no ICE
restart or renegotiation, and no transport-owned packet-loss concealment.

## Possible later bindings

Later work is intentionally versioned rather than silently added to v1:

- A coexisting `webrtcws.v2` for Opus/higher-rate audio, trickle ICE, restart, renegotiation, and
  multiple media streams where measured deployment evidence warrants the complexity.
- QUIC and SIP bindings using the same semantic session runtime model.
- Additional versioned catalogs and negotiated profiles, without changing frozen `rtvbp.v1`
  behavior.

New wire behavior is introduced through a new catalog, envelope, or profile. The deployed
`babelforce.v1` catalog and `classic.v1` envelope remain frozen for compatibility.
