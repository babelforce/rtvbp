---
id: R-12
title: Port the examples and prove interop against rtvbp-go v0.37
pillar: Proof
status: done
priority: 13
design: docs/designs/conformance.md
epic: conformance
areas: [sdk-go, conformance]
note: live bidirectional v0.37.2 WebSocket interop now proves deployed peers remain compatible
---

# Port the examples and prove interop against rtvbp-go v0.37

## Goal
Show the new SDK works as a real client and a real server, and prove that peers running the deployed
implementation keep working against it — which vectors alone cannot establish.

## Acceptance
- [x] The demo client (voice role, dummy phone), the demo server (application role) and the load
      test are ported to the new SDK and run.
- [x] All three are `goleak`-clean.
- [x] R-17's pinned v0.37.2 comparison remains in the conformance gate and covers every common
      payload/envelope shape exercised by interop; any newly added fixture is explicitly classified
      rather than silently skipped.
- [x] An interop test stands the new SDK against the **published** `rtvbp-go v0.37` over a real
      WebSocket and completes a session in **both** role directions: new-as-application against
      old-as-voice, and new-as-voice against old-as-application.
- [x] The interop test covers a full lifecycle: initialize with codec negotiation, audio flowing both
      ways, a `dtmf` event, an idle period longer than the old application-ping interval, and
      termination, proving compatibility after keepalive and termination behavior changes.
- [x] A client that sends no `Sec-WebSocket-Protocol` header is accepted and treated as `rtvbp.v1`
      (test) — the backward-compatibility guarantee.

## Progress
- 2026-08-03: Started after generated conformance vectors closed. Auditing the published v0.37.2
  client/server lifecycle, legacy application pinger, static binary audio, headerless handshake and
  terminal close ordering before adding the live two-direction interoperability module.
- 2026-08-04: Added a standalone interop module pinned to the unmodified published v0.37.2 dependency.
  Both role directions repeatedly complete codec negotiation, exact 320-byte binary audio round
  trips, session updates, DTMF, supported termination and clean shutdown; the legacy voice client
  also completes its application ping after the configured idle interval, and both legacy clients
  exercise a headerless WebSocket handshake.
- 2026-08-04: Ran the demo server and no-device dummy-phone client together through initialize and
  timed hangup, ran the load test, and added `goleak`-guarded wiring tests to both nested demo
  modules. Fixed their `/ws` path mismatch, no-audio nil stream wiring and client `os.Exit` cleanup;
  the pinned fixture comparison and all three module suites pass.

## Notes
- Interop needs the old module from the Go proxy; if CI cannot reach it, vendor v0.37 and say so in
  the CI config rather than silently dropping the test.
- The examples are the first outside consumers of the generated role interfaces — friction here is a
  signal about the emitted API, worth recording in the design doc.
