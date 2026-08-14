# Review: soundness & correctness of implementation and plans

**Date:** 2026-08-03 · **Scope:** committed spec workspace (through `aa64dfe`), imported Go SDK,
frozen golden fixtures, all design docs and stories (R-1 … R-16) · **Reviewer:** Claude Code session
requested by Timo

**State at review time:** R-3 was being implemented concurrently in the same working tree
(uncommitted changes to `spec/crates/rtvbp-spec-model/` plus the untracked
`rtvbp-spec-babelforce-v1` crate). The workspace-level `cargo test` failure observed is the intended
failing-first (TDD red) state of R-3, not a defect. Findings against in-flight code are marked as
such.

## Verdict

The architecture and its risk ordering are sound: fixtures frozen first (R-1), spec proven against
them before any emitter (R-4), byte order treated as contract (`serde_json/preserve_order` is
enabled at the workspace level). The dependency graph is acyclic, the board matches story
frontmatter, and the golden fixtures are genuinely faithful — every byte was produced by the
published `rtvbp-go v0.40.0` module through its real marshal path, hash-pinned, and regeneration
was re-verified byte-identical during this review.

One planning **blocker** (B1), a set of **should-fixes**, and no correctness defects in the
committed Rust code.

---

## Blocker

### B1 — Four events have no byte authority; R-4 as written cannot close

R-3's acceptance requires modeling nine events, including four that exist only in the Rust port
(`output.transcript.delta`, `output.transcript.done`, `input.transcript`, `agent.tool.call`) — the
same four that `docs/designs/architecture.md:19-21` explicitly calls additive **drift**. The frozen
capture contains exactly five event fixtures (`conformance/babelforce.v1/golden/events/`:
`audio.info`, `audio.speech.started`, `call.hangup`, `dtmf`, `session.updated`), because
`rtvbp-go v0.40.0` does not implement the other four.

Consequences:

- R-4's acceptance ("serializes each canonical example and asserts `bytes ==` the frozen fixture")
  is unsatisfiable for four of nine events.
- No story captures their wire truth from the maintained production source that defines it.
- Downstream, R-6's per-fixture golden tests, R-11's vectors, and R-13's docs would emit four event
  shapes whose bytes were never observed.
- `conformance/babelforce.v1/golden/README.md` claims the fixtures "cover … every event's data",
  which is true only of the v0.40.0 event set, not the catalog being modeled.

The in-flight R-3 test already works around this by byte-checking only the five fixture-backed
events (`catalog_contract.rs`, event filter) — reasonable, but the decision must become explicit in
R-4's scope. Resolve by one of:

1. capture fixtures from the Rust port (new story, or reopen R-1's scope);
2. scope R-4/R-6 byte checks to fixture-backed shapes, in writing; or
3. drop the four events from `babelforce.v1` (they'd join a later catalog).

---

## Should-fix — plans

### S1 — R-16 releases the wrong version

`docs/stories/R-16-ci-drift-gate-and-release.md:25` mandates a final `v0.38.0` rtvbp-go release,
but `v0.40.0` is already published (pinned by R-1/R-2 and the CHANGELOG). A lower version sorts
below the existing tag: module-proxy consumers and `go get -u` would never see the deprecation
README. The final release must be greater than v0.40.0 (e.g. v0.41.0).

### S2 — v0.37 interop target vs v0.40.0 fixture authority: never reconciled

Fixtures are the v0.40.0 wire; every interop/compat statement targets v0.37
(`docs/roadmap.md:29,59`, `docs/designs/architecture.md:115,163`,
`docs/designs/conformance.md:12,68,101`, `docs/designs/go-sdk.md:164`, R-12, R-15). The v0.37
choice is motivated (rtvbp-openai pins v0.37.2, `conformance.md:73`), but no document asserts that
v0.37.x and v0.40.0 speak the same wire. If any byte changed in v0.38–v0.40, the plan proves
byte-identity to a wire deployed peers don't speak, and R-12 could fail with all three fixture
layers green. Add one sentence to `conformance.md` asserting — ideally mechanically checking —
v0.37.2 ≡ v0.40.0 on the wire.

### S3 — `session.terminate` role contradiction; R-3 is encoding an answer right now

- `docs/designs/spec-catalog.md:75` and R-3's acceptance: `session.terminate` is voice→application,
  i.e. `handled_by: Application`. The in-flight R-3 contract test pins `(Application, terminal)`.
- But `docs/designs/go-sdk.md:103` ("voice side 501s `session.terminate`" today → "voice side
  implements `session.terminate` properly") and R-9's acceptance
  (`R-9-session-rewrite-ws-transport.md:28`: "The voice role implements `session.terminate`
  properly rather than answering 501") imply the operation also flows application→voice — i.e.
  `handled_by` should be `Both` (or `Voice`).

One of these is wrong. The answer changes R-3's catalog, R-10's generated role interfaces, and
R-9's tests. Decide before R-3 closes.

### S4 — R-9's acceptance requires R-12's output (ownership inversion)

R-9 acceptance: "…the load test is `goleak`-clean." Porting the load test to the new SDK is R-12's
job, and R-12 transitively depends on R-9 (via R-10). As written R-9 cannot close without work a
downstream story owns. Move the load-test criterion to R-12, or scope R-9's criterion to the
runtime's own tests.

### S5 — R-14's blocked-on list is incomplete

R-14's note says "blocked on R-10", but its acceptance requires demo.v1 emitted through "the same
path … types, envelope binding, role glue, docs and vectors" and negotiation rules "documented on
the … profiles page" — artifacts that exist only after R-13 (docs emitter + profiles page) and R-11
(vectors emitter). Add R-11 and R-13 as blockers, or trim the acceptance to what R-10 enables.

### S6 — R-7's blocked-on list omits R-8

R-7's Notes admit the generated codec implements the hand-written `Envelope`/`ControlFrame`/
`WireError` types "(R-8)", yet frontmatter/board say "blocked on R-6" only. Harmless in practice
(R-8 is ready and will land early), but the declared graph permits R-7 before its interfaces exist.

### S7 — the Go module rename is unowned

R-2's note defers renaming to `github.com/babelforce/rtvbp/sdk/go` "until R-8/R-9", but neither
R-8's nor R-9's acceptance mentions it — while R-6 emits packages (with import-path-bearing
generated tests) into `sdk/go/catalog/…` and R-15 requires building against the new path. Assign
the rename explicitly; R-8 is the natural home.

---

## Should-fix — fixture coverage (fidelity is fine; variants are unpinned)

The capture tool is authentic: it depends on the published module (hash-pinned via go.sum; proxy
metadata records origin hash `9370abb…`, matching `golden/README.md`), marshals real SDK structs
via the exact production constructors (`proto.NewRequest`/`.Ok()`/`.NotOk()`/`proto.NewEvent`), and
`TestCaptureReproducesEveryGoldenByte` regenerated all 29 fixtures byte-identically during this
review with `GOPROXY=off`. The imported `sdk/go` tree is byte-identical to the proxy extraction of
v0.40.0 (repo-only extras: the two nested example modules, legitimately excluded from module zips).

However, the fixtures pin only "present" variants. A reimplementation could pass every current
golden check while diverging on exactly these shapes:

1. **No envelope fixture pins a request with `params` present.**
   `envelope/classic.v1/request.json` uses `session.get` with nil params, so the most common
   deployed frame shape — the `"params"` key and its position after `"method"`
   (`sdk/go/proto/request.go:13-18`) — has no envelope-level golden witness. Payload fixtures exist
   only detached from their envelope.
2. **The error envelope pins only the `code:400`, `"any"`-present case.** Every error the SDK
   actually mints via `NewError`/`NotImplemented`/`BadRequest` (`sdk/go/proto/error.go:45-59`) has
   `Data == nil`, producing `{"code":501,"message":"…"}` with no `"any"` key — including the SDK's
   only hard-coded deployed error, the 501 reply to `session.terminate`
   (`sdk/go/proto/protov1/handler.go:170`). The `any`-absent shape and codes −1/500/501 are
   unpinned.
3. **Absent-case omitempty variants are unpinned across payloads:** `ping` without `rtt`/`data`
   (the first real ping has `RTT=0` → `rtt` omitted, `protov1/ping.go:16-18`);
   `ApplicationMoveRequest` with both fields empty → `{}`; `RecordingStartRequest` without tags →
   `{}`; `CallHangupEvent{}` → `{}` (exactly what `fakeHangup` emits, `protov1/telephony.go:96`).
   The reason asymmetry (`CallHangupRequest.reason` required vs `CallHangupEvent.reason` omitempty)
   is only half-pinned.
4. **`audio.info` is all zeros, hiding float formatting.** `bytes_per_second` is `float64`
   (`protov1/audio_info_event.go:13`). Go emits `12800` for 12800.0; serde_json emits `12800.0`
   for the same f64 — invisible at zero. Add fixtures with a whole-valued float and a
   non-terminating one (e.g. 106.66666666666667) to pin Go's shortest-representation behavior.
   This one can genuinely bite R-4/R-6.
5. **Typed-nil result gotcha unpinned.** A handler returning `(nil *T, nil)` reaches `req.Ok(res)`
   with a non-nil interface holding a nil pointer → `"result":null` on the wire despite `omitempty`
   (`sdk/go/proto/response.go:11`). Neither `"result":null` nor a result-absent success response
   appears in any fixture.
6. **`session.terminate.response.json` (`{}`) is not producible by the client SDK**, which answers
   with the 501 error; it matches the in-repo demo server
   (`sdk/go/examples/rtvbp-demo-server/main.go:143-144`). SDK-blessed, but its authority is the
   demo application, not the frozen client code. (Interacts with S3.)

Adding these variant fixtures now via the existing capture tool is cheap and strengthens R-4/R-6
exactly where the presence model is riskiest — which `architecture.md` itself names "the whole
bet."

Verified-clean envelope quirks, for the record: `version` always the string `"1"`; ids are strings
(Go's parser rejects numeric ids — a decode-side constraint emit-only fixtures cannot express;
covered by the Rust codec tests); responses carry no own `id` (correlation via `"response"`);
structural discrimination precedence event → method → response matches `proto/message.go:102-107`
and is mirrored by `spec/crates/rtvbp-spec-model/tests/classic_v1.rs`; `error.data` serializes
under `"any"`; embedded `messageBase.Version` marshals first; `metadata` nil map → `null`;
`audio_codec` nil pointer → `null`; `EmptyResponse{}` → `{}`. Coverage vs v0.40.0 is complete: 10
operations ↔ 10 request+response fixture pairs, 5 events ↔ 5 event fixtures.

---

## Implementation findings — Rust spec workspace

The committed model and `classic.v1` codec are clean: the codec round-trips all four golden
envelopes byte-exactly, and the frozen quirks above are pinned by
`spec/crates/rtvbp-spec-model/tests/classic_v1.rs`. Points on the in-flight (uncommitted) R-3 code:

### I1 — Field order is byte-pinned for only one type (in-flight test gap)

`catalog_contract.rs::canonical_deployed_examples_preserve_frozen_field_order_and_bytes`
serializes the raw example `Value`s — that proves the *examples* match the fixtures, not that the
*typed structs* serialize in the right order. Only `SessionInitializeRequest` gets a direct
struct-to-bytes check. Catalog validation compares round-tripped values with order-insensitive
`Value` equality, so a struct with wrong field declaration order passes every current test,
deferring detection to R-4/R-6.

Cheap, high-value fix: also byte-compare `type_ref.round_trip(&example)` output against the
fixture. With `preserve_order`, the round-tripped value carries struct declaration order, which
mechanically pins the R-3 acceptance item "field declaration order matches the current Go structs"
for every type that has a fixture.

### I2 — Codec does not enforce result/error mutual exclusion

`EnvelopeSpec::encode` will emit a Response carrying both `result` and `error`, and `decode`
accepts one (`spec/crates/rtvbp-spec-model/src/envelope.rs`). Probably acceptable for a reference
codec, but it should be an explicit decision — the deployed protocol presumably treats them as
exclusive.

### I3 — `decode_error` is stricter than the observed wire warrants

It rejects `code == 0` and empty messages (`envelope.rs::decode_error`). All known Go-minted codes
are non-zero (−1/400/500/501), so this is likely safe, but it is an invented constraint; if it is
meant to be contract, it belongs in the spec prose, not only in the codec.

---

## Notes

- **N1 — Drift can merge until R-16.** Vision principle 5 says "Drift cannot merge", yet CI arrives
  only at the end of M1; the roadmap admits no gate exists. R-5 already delivers `--check` — a
  minimal CI (cargo test + go test + `--check`) folded into R-5/R-6 would enforce the principle ten
  stories earlier at near-zero cost.
- **N2 — Gate wording vs reality.** `go test ./...` in `sdk/go` fails out of the box: the example
  modules' `replace` directives point at the old repo layout
  (`sdk/go/examples/rtvbp-demo-{client,server}/go.mod`: `=> ../../../rtvbp-go`, which no longer
  exists; in the monorepo it would be `../..`), and `go.work` pulls those modules in. R-2's notes
  document the `GOWORK=off` workaround, but AGENTS.md's gate says `go test ./...` plainly — R-16
  must reconcile this, or the two `go.mod`s get a one-line fix now.
- **N3 — R-9's "not a single byte on the wire" claim is true only of encodings.** Dropping the
  automatic 10s ping and answering `session.terminate` with success instead of 501 both change
  observable traffic. Question for owner: can a v0.37 peer's liveness depend on receiving app-level
  pings? If plausible, R-12's interop lifecycle should include an idle period.
- **N4 — No error-code registry.** `WireError.Code`, the 501 convention, and the `"any"` key are
  wire-visible, but no spec construct or story declares the error-code space; docs-gen emits only
  the envelope error *shape*. Under vision principle 1, this is a spec-surface gap.
- **N5 — `transport.*` reservation never reaches the published spec.** `architecture.md:154` calls
  writing it into the public spec a must; R-8 reserves it only in code comments "and, later, in the
  published spec" — no story (including R-13) owns the "later".
- **N6 — "profiles page" wording conflict.** `multi-catalog.md:71-72` and R-14 call it *generated*;
  `docs-gen.md:42-45` and R-13 define it as one of the two *hand-written* pages.
- **N7 — Undocumented ordering invariant.** On the deployed Go wire, map-derived objects are always
  key-sorted (`session.get` result) while struct-derived objects are declaration-ordered (`dtmf`).
  The spec side reproduces this correctly via `preserve_order` + literal insertion order, but the
  invariant is nowhere written down — worth a line in the golden README. Likewise unknown-field
  tolerance (Go std json ignores extras) is undocumented by the fixture set.
- **N8 — Bookkeeping.** R-1 is claimed by two design docs' Stories headers (`spec-catalog.md:3` and
  `conformance.md:3`; frontmatter says conformance). `architecture.md:141` says "~11 operations";
  the catalog has 10. Backlog stories R-4–R-7 and R-9–R-16 carry no `priority` frontmatter although
  AGENTS.md's loop selects "top ready story by priority" — this bites the first time two stories
  become ready simultaneously (R-4+R-5 when R-3 lands; R-11/R-12/R-14 when R-10 lands).

## What checked out clean

- Dependency graph is a DAG; board status lists match story frontmatter; roadmap Status matches
  both.
- R-14 "deliberately before the Rust SDK" is consistent (the Rust SDK has no story to order
  against).
- R-4's declared inputs exist: golden fixtures on disk (all four envelope frame shapes including
  the `"any"` error case) and the `classic.v1` reference codec.
- Fixture-first ordering (R-1 before R-3/R-4) is sound and consistently argued.
- `serde_json` `preserve_order` is enabled workspace-wide (`spec/Cargo.toml:15`) — the byte-order
  testing approach is valid.
- Capture-tool authenticity and v0.40.0 provenance of `sdk/go` (see fixture section above).
- `GOWORK=off go test ./...` passes in `sdk/go`; the committed spec-model tests pass
  (`cargo test -p rtvbp-spec-model`: 6/6).

## Priority

The most time-sensitive item is **S3** (`session.terminate` role) because R-3 is encoding an answer
in the working tree right now. **B1** is the one structural hole in the M1 plan. Everything else is
wording, scope bookkeeping, or cheap fixture hardening best done before R-4.
