---
id: R-21
title: Restore generated flows and integration documentation
pillar: Generator
status: done
priority: 18
design: docs/designs/docs-gen.md
epic: docs-gen
areas: [generator, website, sdk-go]
note: generated scenario flows, split quickstarts, and tested deployment auth restore the narrative layer
---

# Restore generated flows and integration documentation

## Goal
Restore the connective tissue lost when the stale hand-written v1 reference was replaced: truthful
session flows, role and transport orientation, a runnable Go path, and deployment-specific
authentication guidance, while keeping every protocol-derived fact generated from the typed spec.

## Acceptance
- [x] Every typed conformance scenario has required prose metadata and a generated Mermaid flow page
      with correct request, response, and event directions plus reference links.
- [x] A typed barge-in scenario proves `audio.speech.started` followed by
      `audio.buffer.clear` through both Go roles.
- [x] The site offers separate first-run paths for Go SDK users and independent protocol
      implementers, with accurate terminology, layers, framing, lifecycle, and use-case context.
- [x] The generic WebSocket binding stays deployment-neutral; a separate babelforce Cloud guide
      documents the deployed RS256 claim contract, public-key handling, and pre-upgrade rejection.
- [x] The Go quickstart and babelforce authentication examples compile and have focused tests; no
      credentials, production private keys, or OpenAI implementation enter this repository.
- [x] Superseded claims such as `session.terminated`, "the application emits no events", and
      unproven media guarantees are not restored.
- [x] Generated drift checks, Go tests, and the Docusaurus production build pass through
      `task check`.

## Progress
- 2026-08-04: Started after auditing the pre-R-13 site against the generated reference. The current
  site has more reference coverage but lost lifecycle, terminology, authentication, and first-run
  context; several old claims were also stale and must not be copied back.
- 2026-08-04: Added required scenario and case descriptions, generated five Mermaid flow pages
  across both catalogs, and added a typed barge-in scenario executed through both Go roles. Restored
  concepts and split quickstarts, added a compile-tested Go endpoint, and documented generic versus
  babelforce-specific authentication with a focused RS256 validator example. Rust tests, generation,
  Go tests, the authentication module, and the Docusaurus production build pass.

## Notes
- Authentication is intentionally two-layered: generic bearer mechanics belong to the WebSocket
  binding, while babelforce issuer, subject, key, and audience behavior belong to a deployment guide.
- The public `sdk/go/v0.1.0-rc.1` tag remains immutable. This story updates Pages from `main` before
  R-16 publishes the stable SDK.
