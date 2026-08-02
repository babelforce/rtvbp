# Design: generated protocol documentation

**Status:** accepted · **Pillar:** Generator · **Stories:** R-13

## Why

The published protocol documentation is hand-written prose about a protocol defined elsewhere.
Those ~650 lines at <https://babelforce.github.io/rtvbp/> are the only spec an integrator sees, and
they already disagree with the implementations in places (it states the application side "is not supposed to send any events", yet
`audio.speech.started` and the transcript events are emitted from exactly there). Prose written by
hand about a protocol defined elsewhere will always drift.

Reference documentation is a **projection of the catalog**, so the generator must emit it.

## Approach

The Docusaurus site lives at `website/` (moved out of `docs/`, which is now the contributor docs and
backlog). Generated reference pages land in `website/docs/reference/<catalog-id>/`:

```
website/docs/reference/babelforce.v1/
  operations/<method>.mdx     # params + result field tables (types, presence, descriptions),
                              #   JSON examples, and a direction badge "voice → application"
  events/<name>.mdx           # data field table, example, emitting role
  roles/application.mdx       # must implement · may call · emits · receives
  roles/voice.mdx             #   (the OpenAI-Realtime client-vs-server framing)
  envelopes/classic-v1.mdx    # frame layout, discrimination order, id scheme,
                              #   error shape including the "any" key
  _category_.json             # so a second catalog version gets its own tree automatically
```

Field tables, type names, presence (required / omitted-when-absent / nullable) and prose all come
from the same schemas and `///` doc comments that produce the SDK types, so the documentation cannot
describe a protocol the SDKs don't implement. Examples are the catalog's canonical examples — the
same values the conformance vectors and the byte-identity tests use.

Every generated file carries a DO-NOT-EDIT banner. Hand-written narrative (the introduction,
transport binding guides, the profile/negotiation page, the outlook) stays hand-authored and links
into the generated reference. The existing `website/docs/protov1/` prose is superseded page by page
as the generated reference lands, and what remains shrinks to narrative only.

Two hand-written pages are new and important, because they are what the north star promises an
integrator: a **transport binding** page (WebSocket for M1 — framing, auth, subprotocol) and a
**profiles & negotiation** page explaining that a profile is *(transport, envelope, catalog)*, that
absence of a subprotocol means `rtvbp.v1`, and which combinations are supported today.

## Alternatives considered

- **Emit AsyncAPI/OpenAPI and render it with off-the-shelf tooling.** The deleted `rtvbp-spec` crate
  did emit AsyncAPI. Generic renderers cannot express the role split, which is the single most
  useful thing a reader needs. We may still emit AsyncAPI as an artifact for external consumers.
- **Keep the reference hand-written and lint it against the catalog.** A linter that understands the
  prose well enough to check it is harder than generating the prose.

## Risks & open questions

- MDX escaping of JSON examples and generated tables needs care; the docs build in CI is the check.
- Sidebar wiring must stay stable as catalogs are added — generate a per-catalog sidebar fragment
  rather than rewriting `sidebars.ts`.

## Acceptance / done

`rtvbp-spec-gen --emit=docs` produces the reference tree; the Docusaurus site builds in CI; every
operation, event, and role of `babelforce.v1` has a generated page; regenerating produces no diff.
