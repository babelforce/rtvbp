# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) per published component
(`spec/`, `sdk/go/`, …).

## [Unreleased]

### Changed

- Moved the Docusaurus site from `docs/` to `website/` so `docs/` can hold the contributor docs and
  the track backlog; the GitHub Pages workflow now builds from `website/`.

### Added

- The track backlog framework: [vision](docs/vision.md), [roadmap](docs/roadmap.md), the
  [board](docs/stories/README.md), and the design records for the spec-first re-implementation.
- Frozen `babelforce.v1` golden wire fixtures captured from `rtvbp-go v0.40.0`, with a disposable
  capture tool and byte-exact regression tests.
- The `rtvbp-go v0.40.0` SDK imported under `sdk/go/` with its history preserved, plus the Rust
  specification workspace, shared catalog model, presence-aware `Nullable<T>`, and byte-exact
  `classic.v1` reference envelope codec.
- The complete typed `babelforce.v1` catalog: ten operations, nine events, roles, terminality,
  canonical examples, target-language type hints, and typed catalog validation.
- Bidirectional spec-side byte parity for all 29 frozen `babelforce.v1` fixtures: every payload and
  event passes through its concrete Rust type, every `classic.v1` envelope decodes and re-encodes
  identically, and an inventory guard prevents fixture coverage from drifting silently.
- Expanded `babelforce.v1` wire authority to 46 source-pinned fixtures: four additive Rust event
  payloads plus Go presence, error, null-result, and float variants. A separate v0.37.2 capture
  proves all 40 shapes shared with v0.40.0 byte-identical and classifies every exclusion.
- Formalized frozen v1 semantics: `session.terminate` remains voice→application, responses allow
  both or neither of result/error, error codes are an open non-zero integer space with four named
  conventions, and catalog validation reserves `transport.*` for transport signaling.
- Added the deterministic `rtvbp-spec-gen` load → validate → resolve → emit pipeline, its committed
  `babelforce.v1` catalog manifest, non-mutating drift checks, stale-output synchronization, and a
  minimal Rust CI gate that prevents spec/generated drift from merging.
