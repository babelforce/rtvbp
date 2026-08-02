---
id: R-3
title: Model babelforce.v1 in the spec crate
pillar: Spec
status: backlog
design: docs/designs/spec-catalog.md
epic: spec-catalog
areas: [spec]
note: blocked on R-2 (spec workspace)
---

# Model babelforce.v1 in the spec crate

## Goal
Port the frozen payload catalog into `rtvbp-spec-babelforce-v1` as typed Rust — every operation,
every event, their roles, and canonical examples — so it can become the single source of truth for
every SDK and every document.

## Acceptance
- [ ] All ten operations are declared with their handling role: `session.initialize` and
      `session.terminate` (application), `session.set`, `session.get`, `application.move`,
      `call.hangup`, `audio.buffer.clear`, `recording.start`, `recording.stop` (voice), and `ping`
      (both).
- [ ] All events are declared with their emitting role: `session.updated`, `dtmf`, `call.hangup`,
      `audio.info` (voice); `audio.speech.started`, `output.transcript.delta`,
      `output.transcript.done`, `input.transcript`, `agent.tool.call` (application).
- [ ] Field presence uses `T` / `Option<T>` / `Nullable<T>` deliberately per field, matching the
      captured fixtures — notably `SessionInitializeRequest.metadata` and
      `SessionInitializeResponse.audio_codec` are `Nullable`.
- [ ] Field declaration order matches the current Go structs (it is the wire's order).
- [ ] `application.move`, `call.hangup` and `session.terminate` are flagged `terminal`.
- [ ] Every operation and event carries a doc comment and at least one canonical example.
- [ ] `catalog()` validates: unique names, every operation and event has a role, every example
      round-trips its own schema.

## Progress
- (not started)

## Notes
- Port field-for-field from `rtvbp-go/proto/protov1/*.go`. Prose for doc comments can be lifted from
  `website/docs/protov1/` and the existing Rust port at
  `private-source.invalid/crates/rtvbp/src/protov1.rs`.
- Go type hints where the current API is not `int64`: `AudioCodec.sample_rate` is Go `int`, whereas
  `DtmfEvent.pressed_at` / `released_at` are `int64`. Use the namespaced `x-go-type` extension.
- `ping` is an ordinary catalog operation — do not lift it to a framework concern.
- `session.get` returns a bare open map as its `result`; model it as-is.
