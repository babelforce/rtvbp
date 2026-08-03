---
id: R-14
title: demo.v1 catalog and a dual-profile example
pillar: Spec
status: backlog
priority: 15
design: docs/designs/multi-catalog.md
epic: multi-catalog
areas: [spec, generator, sdk-go]
note: blocked on R-11; exercises the completed role/docs emitters and generated vectors together
---

# demo.v1 catalog and a dual-profile example

## Goal
Prove the framework is genuinely catalog-agnostic while it is still cheap to fix if it is not — by
adding a second, throwaway catalog and serving it beside the frozen one over a single endpoint.

## Acceptance
- [ ] `rtvbp-spec-demo-v1` defines a minimal catalog (one operation, one event) with both roles
      represented.
- [ ] The generator emits it through exactly the same path as `babelforce.v1` — types, envelope
      binding, role glue, docs and vectors — with **no catalog-specific branches** in the emitters.
- [ ] A Go example serves both `babelforce.v1` and `demo.v1` on one WebSocket endpoint, selecting the
      handler set by negotiated subprotocol.
- [ ] A client of each profile completes an exchange against that server (test).
- [ ] Absence of a subprotocol still selects `rtvbp.v1` (test).
- [ ] Adding the second catalog required no change to the session runtime, the envelope codec, or the
      transport — and if it did, the change is recorded in the design doc as an abstraction fix.

## Progress
- (not started)

## Notes
- Settle the subprotocol naming convention for non-default profiles here and document it on the
  hand-written profiles page from R-13.
- Keep the multi-catalog routing seam in the example, not the runtime, until a second *real* catalog
  exists.
