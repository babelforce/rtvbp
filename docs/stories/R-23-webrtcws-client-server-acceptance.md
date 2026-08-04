---
id: R-23
title: WebRTC plus WebSocket client/server integration and acceptance
pillar: Proof
status: done
priority: 20
design: docs/designs/webrtc.md
epic: webrtc
areas: [sdk-go, conformance]
note: combined endpoints preserve WebSocket audio and prove typed control plus duplex Pion audio
---

# WebRTC plus WebSocket client/server integration and acceptance

## Goal
Make the composite transport usable from normal Go sessions and prove that it is truly WebRTC—not
a WebSocket-audio lookalike—through a full client/server exchange.

## Acceptance
- [x] `webrtcws.Client` is an `rtvbp.Option`; an accepted-transport decorator integrates with the
      existing WebSocket server without duplicating upgrade, auth, profile, queue, or keepalive code.
- [x] The binding selects `rtvbp.webrtc.v1`; incompatible or absent profile offers are rejected
      without weakening the existing `rtvbp.v1` compatibility behavior on plain WebSocket servers.
- [x] WebRTC is additive: the existing `ws.Client`, `ws.Server`, and WebSocket-binary audio path
      remain public, unchanged by default, and covered by their existing tests.
- [x] Authentication still runs before WebSocket upgrade and before any SDP/ICE work (test).
- [x] A leak-clean loopback test completes a typed request/event flow and sends non-silent audio in
      both directions through Pion, asserting PCMU selection and timed RTP-derived frames.
- [x] Failure and shutdown tests cover ICE negotiation timeout, remote WebSocket close, WebRTC
      failure, terminal-response flush, and server shutdown with active composite sessions.
- [x] `go test -race ./...` passes for the Go module after the integration.

## Progress

- 2026-08-04: Added a detached WebSocket construction seam and authenticated accepted-transport
  decorator, then implemented explicit WebRTC client selection and additive combined-server
  profiles. Real sessions now prove typed ping, terminal-response flush, PCMU RTP audio both ways,
  PTS, incompatible-profile rejection, remote shutdown, and continued plain WebSocket operation.
- 2026-08-04: Ran the existing demo pair twice against the same combined server: once with Pion
  WebRTC audio and once with WebSocket-binary audio. Both completed `session.initialize`, the timed
  dummy-phone hangup, terminal response, session removal, and graceful server shutdown.

## Notes
- Tests must fail if audio is accidentally carried as WebSocket binary frames.
- Test configurations use host candidates only; deployed STUN/TURN behavior is configuration, not a
  dependency on a public service in CI.
