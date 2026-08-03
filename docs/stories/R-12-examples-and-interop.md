---
id: R-12
title: Port the examples and prove interop against rtvbp-go v0.37
pillar: Proof
status: ready
priority: 13
design: docs/designs/conformance.md
epic: conformance
areas: [sdk-go, conformance]
note: R-10 ported the demos; published-v0.37 interop now proves deployed peers are unaffected
---

# Port the examples and prove interop against rtvbp-go v0.37

## Goal
Show the new SDK works as a real client and a real server, and prove that peers running the deployed
implementation keep working against it — which vectors alone cannot establish.

## Acceptance
- [ ] The demo client (voice role, dummy phone), the demo server (application role) and the load
      test are ported to the new SDK and run.
- [ ] All three are `goleak`-clean.
- [ ] R-17's pinned v0.37.2 comparison remains in the conformance gate and covers every common
      payload/envelope shape exercised by interop; any newly added fixture is explicitly classified
      rather than silently skipped.
- [ ] An interop test stands the new SDK against the **published** `rtvbp-go v0.37` over a real
      WebSocket and completes a session in **both** role directions: new-as-application against
      old-as-voice, and new-as-voice against old-as-application.
- [ ] The interop test covers a full lifecycle: initialize with codec negotiation, audio flowing both
      ways, a `dtmf` event, an idle period longer than the old application-ping interval, and
      termination, proving compatibility after keepalive and termination behavior changes.
- [ ] A client that sends no `Sec-WebSocket-Protocol` header is accepted and treated as `rtvbp.v1`
      (test) — the backward-compatibility guarantee.

## Progress
- (not started)

## Notes
- Interop needs the old module from the Go proxy; if CI cannot reach it, vendor v0.37 and say so in
  the CI config rather than silently dropping the test.
- The examples are the first outside consumers of the generated role interfaces — friction here is a
  signal about the emitted API, worth recording in the design doc.
