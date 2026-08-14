---
id: R-30
title: Component release notes, artifacts, and provenance
pillar: Proof
status: in-progress
priority: 27
design: docs/designs/releases.md
epic: releases
areas: [release, sdk-go, sdk-rust, spec, conformance]
note: make each component release self-describing, reproducible, checksummed, and attestable
---

# Component release notes, artifacts, and provenance

## Goal
Turn GitHub releases from bare cross-component comparison links into component-scoped, reproducible
distribution records for the Go SDK, Rust SDK, and protocol itself.

## Acceptance
- [ ] Component-owned changelogs generate exact-version release notes and compare only with the
      preceding semantic-version tag in the same namespace, including correct first-release output.
- [ ] A standard-library release tool has failing-first tests for tag/version validation, semantic
      predecessor selection, changelog extraction, deterministic protocol archives, manifests, and
      checksums.
- [ ] Go releases attach a release manifest and checksums; Rust releases additionally attach Cargo's
      verified `.crate`; protocol releases attach a deterministic manifest/conformance bundle.
- [ ] All three workflows gate the repository, build from the immutable requested tag, validate an
      external consumer where applicable, upload exact assets, and create GitHub build-provenance
      attestations over the checksum subjects.
- [ ] Release documentation explains component versioning, contents, checksum and attestation
      verification, recovery behavior, and why Go binaries/source tarballs and website builds are
      not release assets.
- [ ] Existing Go `sdk/go/v0.1.1` and Rust `sdk/rust/v0.1.0` releases are backfilled with accurate
      notes and verified assets, and the initial `protocol/v1.0.0` release is published and verified.

## Progress
- 2026-08-14: Started after auditing the first Go and Rust releases: both asset lists were empty,
  both note bodies were only comparison links, and Rust incorrectly compared from a Go SDK tag.
- 2026-08-14: Added and failing-first tested the standard-library release builder, component
  changelogs, protocol snapshot version, deterministic manifests/checksums/bundle, immutable-tag
  validation, consumer documentation, and all three provenance-enabled workflows. Dry runs against
  the real Go and Rust tags package and checksum successfully; the complete repository gate passes.

## Notes
- Generated protocol artifacts remain derived from the spec; the release tool packages committed
  generated output and never becomes another protocol source of truth.
