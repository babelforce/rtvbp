# Rust SDK changelog

All notable Rust SDK changes are recorded here. Versions correspond to `sdk/rust/v*` tags.

## [0.1.0] - 2026-08-14

### Added

- Published the first Rust SDK generated from the same catalogs and envelope declaration as Go,
  with typed payloads, validation, role adapters, peers, event emitters, and conformance vectors.
- Added the Tokio session runtime, memory and WebSocket transports, the `babelforce.v1` bridge, and
  selectable `rtvbp.webrtc.v1` WebRTC audio with PCMU media.
- Proved both-role wire compatibility with the frozen authority, the published legacy Go SDK, and
  current Go over WebSocket and WebRTC.
