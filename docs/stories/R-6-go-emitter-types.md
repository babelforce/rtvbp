---
id: R-6
title: Go emitter — payload types and name constants
pillar: Generator
status: done
priority: 7
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: 30 generated Go types and 36 exact-byte fixture cases now project the frozen catalog
---

# Go emitter — payload types and name constants

## Goal
Emit the generated Go payload types for `babelforce.v1` and prove they marshal byte-identically to
the frozen fixtures — the moment the "spec generates the SDK" claim becomes real.

## Acceptance
- [x] `--emit=go` writes `sdk/go/catalog/babelforcev1/`: one type per payload, method and event name
      constants, and `MethodName()` / `EventName()` methods.
- [x] Identifiers are idiomatic Go — `DtmfEvent`, not `DTMFEvent`; field `Application`, not
      `AppInfo` — with wire names appearing only in struct tags.
- [x] Presence maps correctly: `T` → value without `omitempty`, `Option<T>` → `omitempty`,
      `Nullable<T>` → pointer **without** `omitempty`.
- [x] Field order in the emitted structs matches spec declaration order.
- [x] Doc comments from the spec appear as Go doc comments.
- [x] Generated golden tests ship alongside the types: for every fixture, unmarshal → re-marshal →
      `bytes.Equal`, and construct-from-code → marshal → equal. They fail first, then pass.
- [x] Output is gofmt-clean and every file carries a DO-NOT-EDIT banner.

## Progress
- 2026-08-03: Started by inventorying the frozen payload/schema shapes and the legacy Go structs;
  defining the failing-first generated package contract before extending the pure emitter pipeline.
- 2026-08-03: Added a validated, target-neutral registry for all 36 payload/event fixtures and gave
  the bare-map `session.get` response an explicit schema identity. The pure Go emitter now produces
  ordered documented types, constants/name methods, and exact-byte construction/round-trip tests;
  generator, Go package, gofmt, and all-target drift checks are passing pending final audit.
- 2026-08-03: Completed after independent schema, legacy-fixture, architecture, and implementation
  audits. All 30 generated types, 10 method bindings, nine event bindings, and 36 fixture cases pass
  Rust validation, exact-byte Go tests, gofmt, all-target drift checks, and the docs build.

## Notes
- Shared named types (`AudioCodec`, `CallInfo`, …) are emitted once and referenced, not duplicated
  per operation.
- The types must be usable before the new runtime exists, so this story depends on nothing in
  `sdk/go` beyond the module itself.
- R-17 must settle the four additive event fixtures and R-18 must formalize frozen semantic
  constraints before this story freezes either shape into generated Go APIs.
