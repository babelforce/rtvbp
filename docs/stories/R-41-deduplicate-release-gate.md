---
id: R-41
title: Validate a release train once and reuse the proven gate
pillar: Proof
status: backlog
priority: 41
areas: [ci, release, supply-chain]
note: stop rebuilding the complete repository gate independently for every component release
---

# Validate a release train once and reuse the proven gate

## Goal
Make coordinated releases fast and legible by proving a commit once, then allowing each component
release to consume that exact successful gate result while still packaging its immutable tag.

## Acceptance
- [ ] A failing-first workflow test proves Protocol, Go, Rust, and TypeScript releases cannot publish
      unless the selected release-automation commit has one successful complete `task check` run.
- [ ] Component release workflows reuse that proof instead of independently executing the full gate;
      an absent, stale, cancelled, or failed proof blocks publication closed.
- [ ] The proof binds the exact commit SHA, lockfiles, generator output, workflow revision, and release
      tag mapping so a later or unrelated green run cannot authorize an artifact.
- [ ] Immutable tagged source remains the packaging input, and all existing checksum, provenance,
      external-install, registry-resolution, and GitHub Release checks remain component-local.
- [ ] Concurrency groups prevent duplicate release trains for the same commit/tags, while retries are
      idempotent and never publish different bytes for an existing version.
- [ ] CI documentation explains the trust boundary, how to trigger a coordinated release, how a
      failed gate is diagnosed, and how an individual component can be retried safely.

## Progress
- Captured after the first four-component release repeated the complete cold gate five times and an
  interop subprocess leak made each redundant job appear hung.
- TypeScript dispatch retries now fail closed unless `spec.yml` has a successful push run for the
  exact release-automation SHA; tag pushes still run the complete gate. Applying the same reusable
  proof to the other component workflows remains backlog work.

## Notes
- Prefer a small reusable workflow or explicit gate workflow result over implicit timing between
  independently dispatched jobs.
- Do not weaken the component-specific immutable-source and public-registry verification steps.
