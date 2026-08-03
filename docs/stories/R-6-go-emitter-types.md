---
id: R-6
title: Go emitter — payload types and name constants
pillar: Generator
status: backlog
priority: 7
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [generator, sdk-go]
note: blocked on R-4, R-5, R-17 and R-18; emits only after wire authority and semantics are complete
---

# Go emitter — payload types and name constants

## Goal
Emit the generated Go payload types for `babelforce.v1` and prove they marshal byte-identically to
the frozen fixtures — the moment the "spec generates the SDK" claim becomes real.

## Acceptance
- [ ] `--emit=go` writes `sdk/go/catalog/babelforcev1/`: one type per payload, method and event name
      constants, and `MethodName()` / `EventName()` methods.
- [ ] Identifiers are idiomatic Go — `DtmfEvent`, not `DTMFEvent`; field `Application`, not
      `AppInfo` — with wire names appearing only in struct tags.
- [ ] Presence maps correctly: `T` → value without `omitempty`, `Option<T>` → `omitempty`,
      `Nullable<T>` → pointer **without** `omitempty`.
- [ ] Field order in the emitted structs matches spec declaration order.
- [ ] Doc comments from the spec appear as Go doc comments.
- [ ] Generated golden tests ship alongside the types: for every fixture, unmarshal → re-marshal →
      `bytes.Equal`, and construct-from-code → marshal → equal. They fail first, then pass.
- [ ] Output is gofmt-clean and every file carries a DO-NOT-EDIT banner.

## Progress
- (not started)

## Notes
- Shared named types (`AudioCodec`, `CallInfo`, …) are emitted once and referenced, not duplicated
  per operation.
- The types must be usable before the new runtime exists, so this story depends on nothing in
  `sdk/go` beyond the module itself.
- R-17 must settle the four additive event fixtures and R-18 must formalize frozen semantic
  constraints before this story freezes either shape into generated Go APIs.
