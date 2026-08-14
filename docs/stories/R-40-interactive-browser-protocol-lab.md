---
id: R-40
title: Build an interactive browser phone and protocol lab in the public docs
pillar: Integration
status: backlog
priority: 40
design: docs/designs/m2-browser-parity.md
epic: m2-browser-parity
areas: [website, sdk-typescript, browser-media, demonstration]
note: let visitors make a safe simulated call and watch negotiation, control, audio, and WebRTC state
---

# Build an interactive browser phone and protocol lab in the public docs

## Goal
Turn the TypeScript/browser SDK into an understandable, hands-on protocol demonstration: a small
phone embedded in the public documentation that can run a complete deterministic fake call without a
backend, and can optionally connect to a visitor-supplied compatible endpoint.

The experience should make the protocol visible rather than merely showing a UI. Visitors can hear
non-silent audio, trigger call actions, and watch negotiation, control frames, lifecycle, media flow,
and WebRTC health change together.

## Acceptance
- [ ] The docs contain an accessible, responsive browser phone with call, mute, DTMF, barge-in,
      clear-buffer, and hangup controls; autoplay, microphone permission, and cleanup states are
      explicit and recoverable.
- [ ] A default simulation runs entirely in the browser with no account, credential, private service,
      or network dependency. It uses generated catalog/envelope APIs, deterministic fake media, and
      representative scenarios rather than a second hand-written protocol implementation.
- [ ] The visualization correlates profile negotiation, session lifecycle, typed requests/responses/
      events, terminal behavior, and audio direction on one timestamped timeline. Payloads are
      redacted by default, and raw inspection is an explicit local-only action.
- [ ] Audio meters or waveforms prove non-silent duplex flow. WebRTC mode shows selected codec,
      connection/ICE state, candidate-pair changes, bitrate, RTT, jitter, and packet loss with clear
      explanations and graceful fallbacks when a browser omits a statistic.
- [ ] An optional live mode accepts an endpoint and caller-provided authentication callback/config at
      runtime. No endpoint, token, deployment-specific OAuth behavior, telemetry, or secret is built
      into source, static assets, examples, logs, screenshots, or analytics.
- [ ] The lab can replay generated conformance scenarios step-by-step and link each visible message to
      its generated reference page, making the specification-to-SDK relationship inspectable.
- [ ] Real-browser tests cover the deterministic simulation, fake-device duplex audio, interaction,
      cancellation, repeated call/hangup cycles, accessibility, and desktop/mobile layouts; the
      ordinary static docs build remains deterministic and usable without JavaScript.
- [ ] A short architecture note defines the trust boundary, sanitization policy, resource ownership,
      live-endpoint threat model, and how the demo stays browser-only while Node server examples live
      elsewhere.

## Notes

- Prefer a “glass box” presentation: phone controls on one side, a synchronized protocol/media
  timeline and stats inspector on the other.
- The offline simulation is the primary public experience. Live connectivity is an advanced mode and
  must fail closed without explicit visitor configuration.
- This follows R-37's browser media/WebRTC adapters and should become part of R-38's public M2 release
  acceptance rather than delaying core runtime correctness.
