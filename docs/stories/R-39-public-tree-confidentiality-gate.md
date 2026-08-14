---
id: R-39
title: Remove private source coordinates and gate the public tree
pillar: Proof
status: in-progress
priority: 0
areas: [release, security, conformance]
note: current tree scrubbed; published-history remediation requires explicit owner authorization
---

# Remove private source coordinates and gate the public tree

## Goal
Ensure this public protocol repository contains no private repository identity, host, path, or SSH
dependency coordinate, and prevent those details from entering future commits or release artifacts.

## Acceptance
- [x] The current public tree replaces private producer coordinates with generic provenance or opaque
      content digests and removes the source-dependent disposable capture.
- [x] The release gate scans tracked and untracked public files for private locators and opaque-denylist
      identifiers without embedding the identifiers in the scanner itself.
- [ ] Published branches, tags, and release artifacts are audited and remediated with an explicitly
      authorized history strategy; no force-push, tag replacement, or release deletion is implicit.

## Progress

- 2026-08-14: A browser-evidence review found that the existing public branch already carried a
  private source coordinate in an older disposable capture. The working tree now retains only the
  immutable public fixture bytes and digest-pinned generic browser evidence.
- 2026-08-14: Added a standard-library gate for private Git transport locators and opaque hashes of
  confidential identifiers. Remote history remains unchanged pending explicit authorization.
- 2026-08-14: Audited every reachable branch and tag: 33 affected historical blob/path pairs span
  twelve paths, while one attached protocol bundle contains one affected member. A disposable mirror
  rehearsal deleted the obsolete capture and replaced three private-locator forms plus two opaque
  identifiers. Every rewritten ref then passed the scanner, the current `main` content tree stayed
  identical, and all six published Go/Rust tag distribution subtrees stayed byte-identical. The
  protocol bundle must be rebuilt and affected releases/source archives reissued after owner-approved
  branch/tag replacement.
