# Releases

RTVBP publishes the Go SDK, Rust SDK, and protocol snapshot independently from one repository. The
tag namespace identifies the component:

| Component | Tags | Distribution |
|---|---|---|
| Go SDK | `sdk/go/v*` | `github.com/babelforce/rtvbp/sdk/go` through the Go module proxy |
| Rust SDK | `sdk/rust/v*` | Git dependency plus an auditable `.crate` release asset |
| Protocol | `protocol/v*` | Manifest and conformance bundle on GitHub Releases |

The workflows test the current repository gate, then build release material from a separate checkout
of the requested immutable tag. A manually recovered release therefore gets current workflow fixes
without silently packaging current `main`.

## Release contents

Every GitHub release includes component-scoped notes, a deterministic JSON release manifest, and a
`SHA256SUMS` file. The manifest records the exact tag and commit, source date, distribution identity,
and catalog hashes.

- Go remains a source module, so its release has no redundant binaries or source archive.
- Rust additionally includes the `.crate` produced by `cargo package --locked` from the tag.
- A protocol release additionally includes a deterministic archive of catalog manifests and their
  conformance fixtures, vectors, and scenarios.
- The website remains a GitHub Pages publication rather than a release archive.

The release workflow creates GitHub build-provenance attestations for every file named by the
checksum list. An SBOM is deliberately deferred until it can contain authoritative package and
license identities instead of an incomplete inferred inventory.

## Verification

Download all assets for a tag, then verify the checksums from inside that directory:

```sh
gh release download protocol/v1.0.0 --repo babelforce/rtvbp --dir rtvbp-release
(cd rtvbp-release && sha256sum -c rtvbp-protocol-v1.0.0-SHA256SUMS)
```

Verify the provenance of an artifact or manifest against this repository:

```sh
gh attestation verify \
  rtvbp-release/rtvbp-protocol-v1.0.0.tar.gz \
  --repo babelforce/rtvbp
```

Release notes compare only with the preceding semantic version in the same tag namespace. This
avoids a Rust or protocol release accidentally inheriting a repository-wide Go comparison.
