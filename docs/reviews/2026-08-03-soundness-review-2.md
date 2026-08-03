# Review 2: re-review after resolution of the 2026-08-03 soundness review

**Date:** 2026-08-03 (afternoon) · **Scope:** commits `aa64dfe..2cf401e` — R-3/R-4 closed, R-17
(wire-authority hardening) and R-18 (frozen semantics) filed and closed, R-5 started · **Reviewer:**
Claude Code session requested by Timo · **Prior review:**
[2026-08-03-soundness-review.md](2026-08-03-soundness-review.md)

## Verdict

**Every blocker and should-fix from the morning review is resolved, and almost all of them are
resolved mechanically — with pinned captures and failing-first tests — rather than by wording.**
All 17 Rust spec tests pass; all three capture tools reproduce their fixtures byte-identically in
fresh (uncached) runs performed during this review; the 46-fixture inventory is exhaustively
classified by source. No new correctness defects found. The residual items below are all minor.

## Prior findings — resolution status

| Finding | Status | How |
|---|---|---|
| **B1** four events without byte authority | **Resolved** | New capture tool pinned to the private Rust source (`private-source.invalid v0.33.0`, rev `408b9bc1`, committed `Cargo.lock`) captures the four additive event payloads through the production `Event::of → Message::Event → to_json_string` path; R-6 is now blocked on R-17/R-18; golden README records the split authority |
| **S1** R-16 releases v0.38.0 | Resolved | R-16 now mandates `v0.41.0` |
| **S2** v0.37↔v0.40 wire never reconciled | **Resolved mechanically** | `capture-rtvbp-go-v0.37.2` regenerates the 40 common fixtures from the published v0.37.2 module and byte-compares them against the golden authority; the six exclusions are classified in an inventory test that fails on any unclassified future fixture (verified: `audio.info` genuinely postdates v0.37.2). R-12 keeps this in the interop gate |
| **S3** `session.terminate` role contradiction | Resolved | Decided `handled_by: Application` from deployed evidence; reverse direction stays the explicit 501 (now a golden envelope fixture); catalog, R-9, R-11 scenarios, go-sdk.md and spec-catalog.md all agree |
| **S4** R-9 requires R-12's output | Resolved | R-9's goleak criterion scoped to the runtime's own tests |
| **S5** R-14 blockers incomplete | Resolved | Now blocked on R-10, R-11 and R-13 |
| **S6** R-7 omits R-8 | Resolved | Now blocked on R-6, R-8 and R-18 |
| **S7** Go module rename unowned | Resolved | R-8 acceptance owns the rename and making `go test ./...` literal |
| **Fixture 1** no request-with-params envelope | Resolved | `request-with-params.json` (session.terminate, pinning `params` position) |
| **Fixture 2** only code-400/`any`-present error | Resolved | `-1`, `500`, `501` frames without `any`, minted via the real `NewError`/`ToResponseError`/`NotImplemented` constructors |
| **Fixture 3** absent-case omitempty variants | Resolved | 5 payload + 2 event variants (`ping` no optionals, `application.move` both empty, `recording.start` no tags, `call.hangup` event `{}`) |
| **Fixture 4** float formatting hidden at zero | Resolved | `audio.info-nonzero.json` pins `12800` and `106.66666666666667`, produced through Go's real decode→marshal path; exposed and fixed a real spec bug (serde would emit `0.0`) and forced `float_roundtrip` on |
| **Fixture 5** typed-nil / absent result unpinned | Resolved | `response-ok-no-result.json` and `response-ok-null-result.json`, including the typed-nil `req.Ok(nilResult)` gotcha; exposed and fixed a second real spec bug (codec used to normalize `result:null` away — `optional_value` no longer drops nulls) |
| **Fixture 6** terminate `{}` authority ambiguous | Resolved | Golden README states it explicitly; the 501 fixture pins the reverse direction |
| **I1** field order byte-pinned for one type only | **Resolved** | `wire_parity.rs` round-trips **every** fixture through its concrete typed struct with byte equality (`TypeRef::round_trip_bytes`), plus an inventory test asserting all 46 fixtures are owned by the proof |
| **I2** result/error exclusion undecided | Resolved | Decided from the deployed Go validator (verified: `Response.Validate` accepts both and neither): both/neither are valid; encode+decode tests pin it |
| **I3** invented error strictness | Resolved | Verified against `ResponseError.Validate` (code 0 = `ErrUnspecified` rejected, empty message rejected) — now authority-backed, enforced symmetrically on encode and decode, tested |
| **N1** no CI until R-16 | Resolved | R-5 acceptance now includes a minimal CI job (Rust tests + `--check`) |
| **N2** gate wording vs `go test ./...` reality | Partially (see R1 below) | R-2 notes the `GOWORK=off` workaround; R-8 owns making the gate literal; R-16 told not to reintroduce the carve-out |
| **N3** traffic-semantics changes vs v0.37 peers | Resolved | R-12 interop now includes an idle period past the old ping interval |
| **N4** no error-code registry | Resolved | `ErrorCodeSpec` registry on `EnvelopeSpec` (−1/400/500/501 as conventions over an open non-zero space), tested; R-13 emits it |
| **N5** `transport.*` reservation unowned | Resolved | Catalog validation rejects `transport.*` operations (tested); R-13 owns publication (see R4 below) |
| **N6** profiles page generated vs hand-written | Resolved | Consistently hand-written (multi-catalog.md, R-14) |
| **N7** ordering/tolerance invariants undocumented | Resolved | Golden README records struct-order vs sorted-map-order, presence behavior, float spellings, unknown-field tolerance |
| **N8** bookkeeping (duplicate story claims, ~11 ops, no priorities) | Resolved | Design Stories headers deduplicated, "10 operations", every story has `priority` |

## What was verified, not just read

- `cargo test` in `spec/`: 17/17 green (catalog contract 6, wire parity 3, classic_v1 4, model
  contract 4).
- `go test -count=1` in both Go capture tools: green — the 42 Go fixtures regenerate byte-identically
  from v0.40.0, and v0.37.2 reproduces all 40 common fixtures byte-identically.
- `cargo test --locked` in the private-source.invalid capture tool: 4/4 green, including byte
  reproduction of the four additive fixtures and provenance pinning (manifest + lockfile assert the
  tag/revision).
- Deployed-behavior claims spot-checked against `sdk/go/proto`: response both/neither permissiveness
  (`response.go`), error code-0/empty-message rejection (`error.go`), `PingResponse.OWD` required vs
  `RTT` omitempty — the spec's presence choices match.
- `audio.info` absent from the v0.37.2 module tree — the comparison's exclusion list is truthful.
- Fixture counts: 46 on disk = 42 Go + 4 Rust; README arithmetic (20+5+10+7) checks out.
- Board status lists match story frontmatter; dependency graph still a DAG (R-17/R-18 → R-6/R-7
  edges added, no cycles).

## Residual / new findings (all minor)

### R1 — AGENTS.md `## Gate` is still not literally runnable (residual N2)

The gate says `cargo test` (works), `task generate && git diff --exit-code` (no Taskfile or
generator exists yet — R-5/R-16), and `go test ./...` (still fails at the repo root: the legacy
`go.work` pulls in example modules whose `replace` directives point at the pre-monorepo layout).
Ownership is now clear (R-8 module paths, R-16 Taskfile), but until R-8 lands, every session
following AGENTS.md verbatim hits a false red. Cheap fix: one parenthetical in the Gate section
pointing at the R-2 note's `GOWORK=off go test ./...` interim form.

### R2 — `serialize_go_float64` has an undocumented exactness envelope

`payloads.rs` reproduces Go's integral-float spelling by casting to `i64`. This is exact for every
plausible `bytes_per_second`, but diverges from Go's `encoding/json` outside a envelope nobody has
written down: integral values above `i64::MAX` (Go `10000000000000000000`, ryu `1e19`), `|v| ≥ 1e21`
(Go `1e+21`, ryu `1e21`), `|v| < 1e-6` (Go `1e-07`, ryu `1e-7`), `-0.0` (Go `-0`, spec `0`), and
non-finite values (Go errors; serde_json emits `null`). Since spec-catalog.md claims "a
Go-compatible serializer for that field", record the envelope in a comment (or clamp with a
`debug_assert!`), so R-11's vector generation doesn't someday synthesize a value outside it.

### R3 — the roadmap Status paragraph is already stale

`docs/roadmap.md` Status says R-3 was the last completed story and "R-4 … R-5 … R-8 are next";
R-4, R-17 and R-18 have since closed and R-5 is in progress. The board's Status narrative is
current. If the roadmap Status is only updated per milestone (R-16 does this), fine — but then it
shouldn't have been rewritten mid-milestone this morning either; pick one cadence.

### R4 — `transport.*` reservation validates operations only

A catalog **event** named `transport.state` passes validation. The reservation is stated as a
*method* namespace, so this may be intended — but the stated purpose ("envelope-independent
transport signaling") plausibly covers events too, and the check is one line. Either extend
`Catalog::validate` to events or state the methods-only scope in spec-catalog.md.

### R5 — `error:null` normalization is the one asymmetry in explicit-null preservation

The codec preserves explicit `result:null` and `any:null` byte-exactly, but decodes
`"error":null` to no-error and re-encodes without the key (pinned by test). Correct — Go's
`omitempty` pointer can never produce `"error":null` — but spec-catalog.md's "preserves explicit
null" bullet lists only `result`/`any` without noting the error-field exception. One sentence.

### R6 — additive-event authority is thinner than the Go authority, by nature

Each of the four Rust-sourced events has exactly one pinned shape; the only optional field among
them (`output.transcript.done.text`) is pinned absent but not present. And reproduction requires
read access to the private GitLab repo — documented honestly in the README, but it means outside
contributors can verify 42 of 46 fixtures, not all. Acceptable for M1; worth remembering when R-13
publishes these shapes as public documentation.

## Note on process

The response to the morning review is a model of how review findings should land: each finding
became either a story acceptance criterion (R-17/R-18, with `design:` pointing at the review), a
one-line doc fix, or an explicit recorded decision — and the two real bugs the new fixtures exposed
(float spelling, `result:null` normalization) were found *because* the coverage findings were
implemented as capture + failing-first tests rather than as prose.
