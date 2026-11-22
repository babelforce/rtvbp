# Go SDK changelog

All notable Go SDK changes are recorded here. Versions correspond to `sdk/go/v*` tags.

## [0.2.0] - 2026-11-22

### Added

- Added the generated `profile` package: the profile tokens, the signaling operation name, `Default`,
  `Headerless`, `ServerPreference()` and `All()` with the transport, envelope, catalog and media
  constraints of every profile, projected from the spec-owned profile registry. The transport
  packages and the dual-profile example now read their profile names from it.

### Fixed

- Inbound audio that reached the transport just before the session shut down (a `session.terminate`,
  a peer close or a local close) is no longer dropped. Shutdown closed the audio read side before
  the media pump had moved the frames still queued in the transport, so a frame received right
  before a terminate never reached `Read`. Reads now return every frame the transport already held
  and then end with EOF; writes stop at the start of shutdown as before. A pump that does not finish
  within the close timeout is cut off.

## [0.1.1] - 2026-08-14

### Changed

- Made the dummyphone example a stateful, race-safe telephony adapter: DTMF is ordered, hangup is
  exactly once, session variables and recordings are safe, and invalid or canceled calls return
  errors.
- Added clean external module resolution to the Go release gate.

## [0.1.0] - 2026-08-13

### Added

- Published the first spec-generated Go SDK with generated payloads, role interfaces, typed peers,
  dispatch glue, the `classic.v1` envelope codec, and conformance vectors.
- Added hand-written session, memory and WebSocket runtimes plus selectable `rtvbp.webrtc.v1`
  WebRTC audio with Pion, including client/server examples and cross-language interoperability.
