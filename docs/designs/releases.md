# Release artifacts and component versioning

RTVBP is one repository with three independently versioned public surfaces:

| Component | Tag namespace | Canonical distribution |
|---|---|---|
| Go SDK | `sdk/go/v*` | Go module proxy |
| Rust SDK | `sdk/rust/v*` | Git tag; packaged `.crate` attached for audit/offline use |
| Protocol snapshot | `protocol/v*` | GitHub release bundle of manifests and conformance material |

The Git tag is immutable. A recovery run may use workflow code from a newer `main`, but every
released package or bundle must be built from a clean checkout of the requested tag and the release
manifest must name that tag's peeled commit.

## Notes

Each component owns a changelog next to its source. The release tool extracts the exact version
section and appends a comparison link only to the preceding tag in the same namespace. GitHub's
repository-wide automatic previous-release selection is not used because it crosses component tag
families.

## Assets

Every release carries a deterministic JSON manifest and `SHA256SUMS`. The manifest records its
schema version, component, version, tag, peeled commit, source date, distribution coordinates, the
SHA-256 digest of every catalog manifest, and the digest of any packaged artifact.

- Go releases attach only the manifest and checksums. The Go proxy remains the canonical package;
  attaching another source archive would create a redundant distribution channel.
- Rust releases attach Cargo's verified `.crate`, the manifest, and checksums.
- Protocol releases attach a deterministic gzip-compressed tar archive containing the catalog
  manifests and catalog-owned conformance fixtures/vectors, plus the manifest and checksums.

The checksum file identifies the subjects of a GitHub build-provenance attestation. Release assets
are created before the public release is finalized, and an existing release can be backfilled
without deleting or moving its tag.

## Version contract

The Rust tag version must equal `sdk/rust/Cargo.toml`. Protocol tags must equal `spec/VERSION`. Go
tags use the module's standard subdirectory semantic version. Stable and prerelease ordering follows
Semantic Versioning, not lexical Git tag order.

## Deliberate exclusions

- SDK binaries: both SDKs are libraries.
- A Go tarball: `proxy.golang.org` already provides the standard `.info`, `.mod`, and `.zip` forms.
- The built website: GitHub Pages is its distribution channel.
- An inferred SBOM: a future SBOM must carry authoritative package identities and licenses; build
  provenance and exact checksums ship now rather than publishing an incomplete dependency claim.
