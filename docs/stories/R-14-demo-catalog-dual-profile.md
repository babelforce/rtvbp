---
id: R-14
title: demo.v1 catalog and a dual-profile example
pillar: Spec
status: done
priority: 15
design: docs/designs/multi-catalog.md
epic: multi-catalog
areas: [spec, generator, sdk-go]
note: demo.v1 now proves common emitters and dual-profile routing with one idempotence fix
---

# demo.v1 catalog and a dual-profile example

## Goal
Prove the framework is genuinely catalog-agnostic while it is still cheap to fix if it is not — by
adding a second, throwaway catalog and serving it beside the frozen one over a single endpoint.

## Acceptance
- [x] `rtvbp-spec-demo-v1` defines a minimal catalog (one operation, one event) with both roles
      represented.
- [x] The generator emits it through exactly the same path as `babelforce.v1` — types, envelope
      binding, role glue, docs and vectors — with **no catalog-specific branches** in the emitters.
- [x] A Go example serves both `babelforce.v1` and `demo.v1` on one WebSocket endpoint, selecting the
      handler set by negotiated subprotocol.
- [x] A client of each profile completes an exchange against that server (test).
- [x] Absence of a subprotocol still selects `rtvbp.v1` (test).
- [x] Adding the second catalog required no change to the session runtime, the envelope codec, or the
      transport — and if it did, the change is recorded in the design doc as an abstraction fix.

## Progress
- 2026-08-04: Started after published-version interop closed. Auditing catalog loading, shared
  envelope projection, generated Go package naming, supported-profile negotiation and the
  example-only routing seam before adding `demo.v1` through the common emitter path.
- 2026-08-04: Added the typed `demo.v1` echo operation, observed event and exchange scenario, then
  loaded it beside the frozen catalog. The unchanged pipeline now commits two manifests, generated
  `catalog/demov1`, six reference pages and three vector files; common-pipeline tests guard every
  target without a demo-specific emitter branch.
- 2026-08-04: Added a dual-profile WebSocket example and leak-clean tests for negotiated
  `rtvbp.demo.v1` echo/event exchange plus headerless `rtvbp.v1` ping. The test exposed and fixed
  empty-slice defaulting idempotence in the transport configuration; no session or envelope code
  changed, and the example retains the only profile-to-handler routing seam.

## Notes
- Settle the subprotocol naming convention for non-default profiles here and document it on the
  hand-written profiles page from R-13.
- Keep the multi-catalog routing seam in the example, not the runtime, until a second *real* catalog
  exists.
