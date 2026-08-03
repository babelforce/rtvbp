---
sidebar_position: 99
---

# Outlook

RTVBP separates payloads, envelopes, and transports so new bindings do not require a new
call-control protocol.

Planned work after the first SDK milestone includes:

- WebRTC audio with WebSocket control for browser and low-latency media use cases.
- A Rust SDK generated from the same catalog and envelope declarations.
- QUIC and SIP bindings using the same semantic session runtime model.
- Additional versioned catalogs and negotiated profiles, without changing frozen `rtvbp.v1`
  behavior.

New wire behavior is introduced through a new catalog, envelope, or profile. The deployed
`babelforce.v1` catalog and `classic.v1` envelope remain frozen for compatibility.
