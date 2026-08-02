---
id: R-1
title: Capture frozen golden wire fixtures from rtvbp-go
pillar: Proof
status: done
priority: 1
design: docs/designs/conformance.md
epic: conformance
areas: [conformance]
note: the authority for every later byte-identity check — capture before the spec is written
---

# Capture frozen golden wire fixtures from rtvbp-go

## Goal
Record the exact bytes the deployed protocol produces today, so the spec can be authored against
observed reality rather than remembered semantics. These fixtures are the authority that every
later byte-identity check compares against.

## Acceptance
- [x] A throwaway capture program, run against the current `rtvbp-go`, emits canonical JSON for
      every operation's params and result, every event's data, and all four envelope frame shapes:
      request, response-ok, response-error, and event.
- [x] The response-error fixture includes populated error data, proving the `"any"` key
      (`proto/error.go:21`) is captured rather than assumed.
- [x] Fixtures cover the presence subtleties explicitly: a `session.initialize` request with absent
      `metadata` and a response with absent `audio_codec` both show `null`, not omission.
- [x] Fixtures land under `conformance/babelforce.v1/golden/`, one file per case, with a README
      stating they are frozen and that changing one means changing the wire.
- [x] The capture program is committed under a clearly disposable path (it depends on the old
      module and is never part of the build).

## Progress
- Capture source pinned to `rtvbp-go v0.40.0` (`9370abb8d18cf3c89837d4d1c63564f6218e354d`).
- Captured 20 operation payloads, five event payloads, and four `classic.v1` envelope frames.
- A failing-first inventory test now proves every committed fixture is deterministic and byte-exact.

## Notes
- Source of truth for shapes: `rtvbp-go/proto/protov1/*.go`; envelope in `proto/message.go`
  (discrimination order **event → method → response**) and `proto/error.go`.
- Also capture the quirks worth pinning: `params` omitted when nil, responses carrying no `id` of
  their own, events carrying an `id` nothing consumes, and `session.get` returning a bare map.
- Independent of R-2 — these two can run in parallel.
