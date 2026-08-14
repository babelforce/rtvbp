---
id: R-31
title: Brand and polish the public protocol documentation
pillar: Integration
status: done
priority: 32
design: docs/designs/docs-gen.md
epic: docs-gen
areas: [website, documentation, branding]
note: replace Docusaurus boilerplate with a clear RTVBP entry point under babelforce stewardship
---

# Brand and polish the public protocol documentation

## Goal
Give integrators a credible, useful first encounter with RTVBP: explain the protocol before the
implementation details, route each audience to the right quickstart, and apply the current
babelforce identity without blurring the distinction between protocol and steward.

## Acceptance
- [x] The public landing page states what RTVBP connects, exposes Go, Rust, and wire-protocol entry
      points, and explains the currently deployed profile without Docusaurus placeholder content.
- [x] Navigation, typography, color, favicon, and steward attribution use current babelforce source
      material while preserving RTVBP as the protocol name.
- [x] Every copied brand asset has an authoritative source URL, retrieval date, checksum, ownership,
      and permitted project use recorded in the repository.
- [x] Generated reference pages remain untouched, edit links target this repository, and external
      links are explicit and accessible.
- [x] TypeScript checking and a production Docusaurus build pass.

## Progress
- 2026-08-14: Started from the visible Docusaurus starter landing page and assets. The current
  babelforce homepage and WordPress media API are the visual authority for the wordmark, mark,
  palette, and typography.
- 2026-08-14: Replaced the empty starter landing page with a responsive protocol overview, deployed
  profile card, three integration paths, compatibility principles, and explicit babelforce
  stewardship. Removed visible starter content and corrected source-edit navigation.
- 2026-08-14: Recorded authoritative URLs, ownership, retrieval date, and byte checksums for every
  embedded brand source. Re-extracting each lossless SVG wrapper reproduces its recorded PNG hash.
  `yarn typecheck` and the production `yarn build` both pass; desktop and mobile headless-browser
  renders were inspected at 1440×1400 and 390×844.

## Notes
- This story changes hand-written site presentation only. `website/docs/reference/` remains an
  immutable generator output.
