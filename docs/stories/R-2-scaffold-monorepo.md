---
id: R-2
title: Scaffold the monorepo — spec workspace and sdk/go subtree import
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/spec-catalog.md
epic: spec-catalog
areas: [spec, sdk-go]
note: brings rtvbp-go history in under sdk/go; the Docusaurus move to website/ is already done
---

# Scaffold the monorepo — spec workspace and sdk/go subtree import

## Goal
Turn this repository from a documentation site into the home of the protocol: a Rust spec workspace,
the Go SDK imported with its history intact, and the shared model crate that later stories build on.

## Acceptance
- [ ] `rtvbp-go` is imported under `sdk/go/` **with history preserved** (`git subtree` or
      `git-filter-repo`); `git log -- sdk/go` shows the original commits.
- [ ] The imported code still builds and its existing tests still pass, untouched — it is the
      reference implementation until the runtime is rewritten.
- [ ] `spec/` is a Rust workspace containing `rtvbp-spec-model`, compiling with `cargo test`.
- [ ] `rtvbp-spec-model` defines `Catalog`, `Operation`, `Event`, `Role`, `TypeRef`, `Nullable<T>`
      and `EnvelopeSpec`, plus a `classic.v1` reference codec in Rust with unit tests.
- [ ] `Nullable<T>` serializes as `null` when empty and marks its schema so emitters can distinguish
      it from `Option<T>` (a unit test pins both behaviours).
- [ ] Root `.gitignore` covers Rust and Go build output.

## Progress
- 2026-08-03: Started; importing the pinned `rtvbp-go v0.40.0` history and scaffolding the Rust
  specification model.

## Notes
- Already done ahead of this story: the Docusaurus site moved `docs/` → `website/` and the Pages
  workflow was updated, freeing `docs/` for the contributor docs and this backlog.
- Go module path is `github.com/babelforce/rtvbp/sdk/go`, root package `rtvbp` — but do **not**
  rename the module in this story; the imported tree keeps building as-is until R-8/R-9.
- Independent of R-1 — these two can run in parallel.
