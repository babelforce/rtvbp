# Go SDK changelog

All notable Go SDK changes are recorded here. Versions correspond to `sdk/go/v*` tags.

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
