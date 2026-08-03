---
id: R-5
title: Generator skeleton and manifest emitter
pillar: Generator
status: ready
priority: 5
design: docs/designs/spec-catalog.md
epic: spec-catalog
areas: [generator]
note: can run alongside R-4; the cheapest emitter forces the model to be complete
---

# Generator skeleton and manifest emitter

## Goal
Stand up `rtvbp-spec-gen` — the pipeline, the CLI, and the deterministic write machinery — and prove
it end to end with its cheapest emitter, which also forces the spec model to expose everything later
emitters will need.

## Acceptance
- [ ] `rtvbp-spec-gen --emit=<target> --out=<dir>` runs the full pipeline: load → validate → resolve
      → emit → write.
- [ ] The validate stage fails loudly on a duplicate method or event name, an operation or event
      without a role, and an example that does not round-trip its schema (a test per case).
- [ ] `--emit=manifest` writes `spec/manifests/babelforce.v1.catalog.json` containing the catalog id,
      every operation and event with its role and terminality, and the embedded schemas.
- [ ] Output is deterministic: stable ordering and a trailing newline, so regenerating twice
      produces no diff (a test asserts this).
- [ ] `--check` re-emits and exits non-zero on any difference, ready for the CI drift gate.
- [ ] Emitters are pure `model → [(path, bytes)]` functions; writing is the only side effect.

## Progress
- (not started)

## Notes
- The generator links the catalog crates and walks the `Catalog` value in process — the manifest is
  an emitted artifact for external tooling and PR review, never the generator's input.
- Keep target-specific concerns out of the model; when a target needs something, prefer a namespaced
  schema extension (`x-go-type`).
