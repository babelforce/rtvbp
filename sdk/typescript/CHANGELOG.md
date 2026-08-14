# TypeScript SDK changelog

All notable TypeScript SDK changes are recorded here. Versions correspond to `sdk/typescript/v*`
tags and public `@babelforce/rtvbp` versions.

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
