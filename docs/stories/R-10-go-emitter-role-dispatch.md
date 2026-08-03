---
id: R-10
title: Go emitter — role interfaces, dispatch adapters and typed peer clients
pillar: Generator
status: done
priority: 11
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: generated role APIs and the v1 voice bridge now replace hand-written protocol glue
---

# Go emitter — role interfaces, dispatch adapters and typed peer clients

## Goal
Make the protocol's role asymmetry visible in the API: each side gets an interface listing exactly
what it must implement, and a typed client for exactly what the other side offers — all generated.

## Acceptance
- [x] `--emit=go` writes `ApplicationHandler` and `VoiceHandler` interfaces containing precisely the
      operations their role handles (plus `Both`), derived from the catalog.
- [x] `ApplicationHandlers(h)` / `VoiceHandlers(h)` adapters bind an implementation into the runtime's
      dispatch, with typed marshalling and validation.
- [x] Typed peer clients are emitted for the operations the *other* role offers, e.g.
      `VoicePeer.CallHangup(ctx, params)` from the application side.
- [x] Event emit and subscribe helpers are emitted per role.
- [x] Unknown method answers 501, unknown event is ignored; both are hookable (tests).
- [x] Operations flagged `terminal` in the spec drive the runtime's respond-then-close path — no
      hand-written per-operation side effects anywhere.
- [x] The voice-side client handler and its telephony adapter are rebuilt on the generated glue,
      replacing the hand-written `protov1.NewClientHandler`, and the ported handler tests pass.

## Progress
- 2026-08-03: Started after the semantic runtime and follow-up soundness review closed. Audited the
  resolved role/terminal metadata, runtime adapter seams, event directions, validation ownership,
  reverse-termination compatibility, and the transitional voice bridge before freezing generated APIs.
- 2026-08-03: Emitted role-exact handlers, typed peers, event surfaces, structured validators,
  terminal adapters and per-role rejections from catalog metadata; synthetic generator contracts and
  generated Go tests pin directionality, validation, terminality, frozen rejection and unknown hooks.
- 2026-08-03: Replaced `proto/protov1` with the generated catalog and a thin hand-written v1 voice
  bridge, ported both demos and handler scenarios, and passed Rust tests, both drift checks, Go tests,
  race/leak coverage, vet, nested demo builds and the Docusaurus production build.

## Notes
- Direction facts to encode: `session.initialize` / `session.terminate` flow voice → application;
  `call.hangup`, `application.move`, `session.set` / `session.get`, `audio.buffer.clear` and
  `recording.*` flow application → voice; `ping` is bidirectional.
- The generated glue plugs into the runtime's existing typed request/event registration; it does not
  replace it.
