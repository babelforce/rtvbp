---
id: R-10
title: Go emitter — role interfaces, dispatch adapters and typed peer clients
pillar: Generator
status: backlog
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: blocked on R-6 and R-9; turns role asymmetry into concrete API surface
---

# Go emitter — role interfaces, dispatch adapters and typed peer clients

## Goal
Make the protocol's role asymmetry visible in the API: each side gets an interface listing exactly
what it must implement, and a typed client for exactly what the other side offers — all generated.

## Acceptance
- [ ] `--emit=go` writes `ApplicationHandler` and `VoiceHandler` interfaces containing precisely the
      operations their role handles (plus `Both`), derived from the catalog.
- [ ] `ApplicationHandlers(h)` / `VoiceHandlers(h)` adapters bind an implementation into the runtime's
      dispatch, with typed marshalling and validation.
- [ ] Typed peer clients are emitted for the operations the *other* role offers, e.g.
      `VoicePeer.CallHangup(ctx, params)` from the application side.
- [ ] Event emit and subscribe helpers are emitted per role.
- [ ] Unknown method answers 501, unknown event is ignored; both are hookable (tests).
- [ ] Operations flagged `terminal` in the spec drive the runtime's respond-then-close path — no
      hand-written per-operation side effects anywhere.
- [ ] The voice-side client handler and its telephony adapter are rebuilt on the generated glue,
      replacing the hand-written `protov1.NewClientHandler`, and the ported handler tests pass.

## Progress
- (not started)

## Notes
- Direction facts to encode: `session.initialize` / `session.terminate` flow voice → application;
  `call.hangup`, `application.move`, `session.set` / `session.get`, `audio.buffer.clear` and
  `recording.*` flow application → voice; `ping` is bidirectional.
- The generated glue plugs into the runtime's existing typed request/event registration; it does not
  replace it.
