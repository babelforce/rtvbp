---
id: R-37
title: Add browser media, WebRTC v1, and deployment auth seams
pillar: SDK
status: in-progress
priority: 38
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [sdk-typescript, webrtc, website, conformance]
note: replace the manual browser client with tested device adapters for both deployed profiles
---

# Add browser media, WebRTC v1, and deployment auth seams

## Goal
Make the generated TypeScript SDK usable for real microphone/speaker sessions over both frozen v1
bindings while keeping AudioContext, WebRTC tracks, and babelforce OAuth policy outside the protocol.

## Acceptance
- [ ] A browser adapter owns permission, AudioContext/AudioWorklet lifecycle, stateful device-rate↔8 kHz
      conversion, playback buffering, barge-in clearing, teardown, and actionable failures.
- [ ] `rtvbp.v1` carries exact L16 binary audio and `rtvbp.webrtc.v1` uses native browser WebRTC with the
      existing bounded offer/answer, PCMU, one-audio-channel, and non-trickle contract unchanged.
- [ ] Callers can inject headers where the platform permits and subprotocol/query/cookie token policy
      where it does not; the babelforce OAuth subprotocol helper and Origin requirements are documented
      as a deployment adapter.
- [ ] Real headless-browser tests use fake microphone media to prove non-silent duplex WebSocket and
      WebRTC audio, barge-in, typed callbacks, cancellation, permission failure, and resource cleanup
      against Go and Rust servers.
- [ ] One bounded real-device smoke test records environment and non-sensitive evidence; current WebRTC
      limits remain conspicuous in generated/profile-backed docs.
