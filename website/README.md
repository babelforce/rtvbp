# RTVBP public documentation

This Docusaurus site is the published integration documentation for RTVBP. It combines hand-written
guides under `docs/` with generator-owned protocol reference under `docs/reference/`.

Never edit `docs/reference/` by hand. Change the protocol specification and regenerate instead; see
the repository [`AGENTS.md`](../AGENTS.md) for the complete contract and gate.

## Work locally

```bash
yarn install --frozen-lockfile
yarn start
```

Run both static checks before submitting presentation or guide changes:

```bash
yarn typecheck
yarn build
```

The repository-level `task check` also verifies generated drift, both SDKs, and conformance.

## Brand assets

RTVBP is the protocol; babelforce is its steward. Keep that distinction explicit in site copy.
Current logo provenance, checksums, ownership, and permitted project use are recorded in
[`static/img/BRAND_ASSETS.md`](static/img/BRAND_ASSETS.md).

## Deployment

GitHub Actions builds and deploys the site to GitHub Pages from `main`. Do not use Docusaurus's
local deployment command for this repository.
