# RTVBP — working agreements

This repository is the **protocol**: a machine-readable specification, the generator that turns it
into SDKs and documentation, and those SDKs. Read [docs/vision.md](docs/vision.md) once; it is the
tie-breaker for design questions.

## The one rule

**The spec is the only source of truth, and generated output is never hand-edited.**

Anything derivable from the spec is emitted by `rtvbp-spec-gen`: SDK payload types, role interfaces
and dispatch glue, typed peer clients, **envelope codecs**, reference documentation, and conformance
vectors. Every generated file carries a DO-NOT-EDIT banner. To change any of it, change the spec and
regenerate.

Hand-written code is reserved for what genuinely cannot be derived: the session runtime, transports,
the audio ring buffer, and each SDK's thin conformance harness.

`babelforce.v1` is **frozen** and must stay byte-identical on the wire, quirks included (see
[docs/designs/architecture.md](docs/designs/architecture.md)). A wire change is a new catalog, never
an edit to an existing one.

## Repository map

| Path | What |
|---|---|
| `spec/` | Rust workspace: the spec model, the catalogs, and the generator |
| `sdk/go/` | Go SDK — hand-written runtime + generated catalog/envelope code |
| `conformance/` | Frozen golden fixtures + generated vectors and scenarios |
| `website/` | The published Docusaurus site; `website/docs/reference/` is GENERATED |
| `docs/` | Contributor docs: vision, roadmap, backlog, design records |

Note the two doc trees: `docs/` is for contributors, `website/` is the published protocol
documentation.

## Gate

Before calling work done: `cargo test` (spec), `task generate && git diff --exit-code` (no drift),
`go test ./...` (SDK), and the docs build. CI runs the same chain — a regenerated diff fails the
build.

<!-- BEGIN track:agents -->
## Start here (every session) — track backlog

This project tracks work with the **track** framework: every unit of work is a markdown story in
`docs/stories/`, and the board (`docs/stories/README.md`) is generated from story frontmatter.

1. **Orient** — read the latest user request, then run `git status --short --branch`. Treat
   uncommitted changes as user-owned unless you made them.
2. **What to work on** — if the user named work, do that. Otherwise open the
   [board](docs/stories/README.md) and take the top `ready` story by `priority` (lower = higher).
   `/track:next` reports it; `/track:next <area>` filters by optional story `areas`.
3. **The contract** — read the story's `## Goal` and `## Acceptance`; Acceptance defines "done". Read
   any linked `design:`.
4. **Do the work** — set the story `in-progress`; non-trivial design goes in `docs/designs/` first;
   implement; satisfy Acceptance with a **failing-first test**; keep the project's gate green.
5. **On done** — `/track:done <ID>`: set `status: done`, add a CHANGELOG entry, regenerate the board.
6. **New or unscoped work?** Create a story first (`/track:story`) so the next agent inherits the
   context.

The board's status lists are generated — after any change to a story's `status`/`priority`/`title`/
`epic`, run `/track:board`. Use optional `areas: [subsystem]` tags for query-only subsystem selection
without changing board rows. Story frontmatter is the single source of truth.
<!-- END track:agents -->
