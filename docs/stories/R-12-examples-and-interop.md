---
id: R-12
title: Port the examples and prove interop against rtvbp-go v0.37
pillar: Proof
status: backlog
priority: 13
design: docs/designs/conformance.md
epic: conformance
areas: [sdk-go, conformance]
note: blocked on R-10; interop is what actually proves deployed peers are unaffected
---

# Port the examples and prove interop against rtvbp-go v0.37

## Goal
Show the new SDK works as a real client and a real server, and prove that peers running the deployed
implementation keep working against it — which vectors alone cannot establish.

## Acceptance
- [ ] The demo client (voice role, dummy phone), the demo server (application role) and the load
      test are ported to the new SDK and run.
- [ ] All three are `goleak`-clean.
- [ ] A capture comparison mechanically proves that the v0.37.2 payload/envelope shapes exercised
      by interop are wire-equivalent to the v0.40.0 authority used by the golden fixtures; the
      result and exact scope are recorded in the conformance design.
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
