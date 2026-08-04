---
id: R-24
title: Document and demonstrate the Go WebRTC binding
pillar: SDK
status: done
priority: 21
design: docs/designs/webrtc.md
epic: webrtc
areas: [sdk-go, website]
note: published binding guide and selectable existing demo pair document both audio choices
---

# Document and demonstrate the Go WebRTC binding

## Goal
Give Go integrators a runnable path to the Pion binding and protocol implementers a precise public
description of its signaling, media, authentication, and deployment behavior.

## Acceptance
- [x] The published transport guide includes a generated-independent Mermaid flow for WebSocket
      upgrade, reserved offer/answer signaling, ICE/DTLS establishment, control, timed RTP media,
      and orderly close.
- [x] The guide documents `rtvbp.webrtc.v1`, PCMU on RTP, L16 little-endian at the SDK boundary,
      non-trickle ICE, STUN/TURN configuration, authentication ordering, and current limitations.
- [x] The existing compile-tested demo client/server gain an audio-transport preference, showing
      both the client option and server decorator, explicit media negotiation, environment-supplied
      ICE configuration, and graceful shutdown without secrets; no duplicate demo pair is added.
- [x] The Go README and getting-started pages link to the binding guide and example without making
      WebRTC the default or changing plain WebSocket instructions.
- [x] The guide shows how callers choose plain WebSocket audio or WebRTC audio at connection setup;
      it does not describe either binding as replacing the other.
- [x] Docusaurus production build and the complete `task check` gate pass.

## Progress

- 2026-08-04: Published the additive binding contract, connection flow, codec boundary, ICE/TURN
  deployment guidance, and limitations. Extended the existing demo client/server with transport
  choice and preference flags plus environment-supplied ICE settings; no duplicate demo was added.

## Notes
- Public prose about the transport is hand-written. Payload operations/events and typed scenarios
  remain generated from the spec.
