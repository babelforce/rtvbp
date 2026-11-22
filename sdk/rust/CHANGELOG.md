# Rust SDK changelog

All notable Rust SDK changes are recorded here. Versions correspond to `sdk/rust/v*` tags.

## [0.2.0] - 2026-11-22

### Added

- Added the generated `profile` module: the profile tokens, the signaling operation name, the
  default and headerless profiles, the server preference order and the descriptors with the
  transport, envelope, catalog and media constraints of every profile, projected from the
  spec-owned profile registry. The WebSocket and WebRTC transports read their profile names from it.

### Fixed

- Inbound audio that reached the transport just before the session shut down (a `session.terminate`,
  a peer close or a local close) is no longer dropped. Shutdown closed the audio read side before
  the media pump had moved the frames still queued in the transport, so a frame received right
  before a terminate never reached the reader. Reads now return every queued frame and then end;
  writes stop at the start of shutdown as before. A pump that does not finish within the close
  timeout is cut off.
- The WebSocket transport no longer reports `Sending after closing is not allowed` as a failed close
  when the peer's Close frame arrives while the local Close is being written. That is an orderly
  close, and `Session::close` now ends cleanly.
- The WebRTC-over-WebSocket codec decodes samples with `as_chunks::<2>()` instead of
  `chunks_exact(2)`, which current stable clippy denies. Behaviour is unchanged.

## [0.1.0] - 2026-08-14

### Added

- Published the first Rust SDK generated from the same catalogs and envelope declaration as Go,
  with typed payloads, validation, role adapters, peers, event emitters, and conformance vectors.
- Added the Tokio session runtime, memory and WebSocket transports, the `babelforce.v1` bridge, and
  selectable `rtvbp.webrtc.v1` WebRTC audio with PCMU media.
- Proved both-role wire compatibility with the frozen authority, the published legacy Go SDK, and
  current Go over WebSocket and WebRTC.
