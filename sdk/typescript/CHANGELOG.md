# TypeScript SDK changelog

All notable TypeScript SDK changes are recorded here. Versions correspond to `sdk/typescript/v*`
tags and public `@babelforce/rtvbp` versions.

## [0.1.1] - 2026-11-22

### Fixed

- Inbound audio that reached the transport just before the session shut down (a `session.terminate`,
  a peer close or a local close) is no longer dropped. Shutdown closed the audio read side before
  the media pump had moved the frames still queued in the transport, so a frame received right
  before a terminate never reached the reader. Reads now return every queued frame and then end;
  writes stop at the start of shutdown as before. A pump that does not finish within the close
  timeout is cut off.
- The Node WebSocket client no longer drops frames the server sends right after the upgrade. The
  adapter handed each frame to its subscribers, but the transport subscribes a few ticks after the
  socket opens, so a first audio frame sent at once was lost and a session whose peer waits for audio
  hung. The adapter now holds frames until the transport subscribes.
- The interoperability tests against the Go and Rust peers build each peer once, stop it on every
  exit path and run one at a time; the package `test` script runs test files serially.

## [0.1.0] - 2026-08-14

### Added

- Added the first spec-generated TypeScript SDK with payloads, validators, classic-envelope
  codecs, role adapters, typed peers, generated profiles, and complete conformance vectors.
- Added browser-neutral session and memory runtimes, browser and Node WebSocket transports, and a
  Node WebSocket server supporting both generated roles.
- Added browser microphone and speaker audio through AudioWorklet, stateful L16 resampling, and
  native WebRTC with PCMU media, explicit authentication injection, cancellation, and bounded
  resource ownership. Capture-level and playback-frame callbacks support application meters and
  activity visualization; the connected transport exposes binding-specific diagnostics such as
  WebRTC statistics without weakening session ownership.
- Proved browser WebSocket and WebRTC interoperability with both Go and Rust using non-silent media
  in real Chrome, plus a bounded real-device microphone and speaker smoke test.
