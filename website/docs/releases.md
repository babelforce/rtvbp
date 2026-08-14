---
sidebar_position: 7
title: Releases and verification
description: Install SDK releases and verify RTVBP protocol artifacts.
---

# Releases and verification

RTVBP has four independently versioned release streams in the same repository. Choose the stream
you consume; a protocol snapshot does not force an SDK upgrade, and one SDK release does not imply
a version change in another language.

| What | Tag family | Canonical distribution |
|---|---|---|
| TypeScript SDK | `sdk/typescript/v*` | npm package `@babelforce/rtvbp` |
| Go SDK | `sdk/go/v*` | Go module proxy |
| Rust SDK | `sdk/rust/v*` | Git tag; packaged `.crate` attached for audit and offline use |
| Protocol snapshot | `protocol/v*` | GitHub release bundle |

Browse every published version on [GitHub Releases](https://github.com/babelforce/rtvbp/releases).
Notes and comparison links stay within their component, so an SDK release never compares itself to
an unrelated protocol or language tag.

## Install an SDK

```sh title="TypeScript / Node 22+ / browser"
npm install @babelforce/rtvbp@0.1.0
```

```sh title="Go"
go get github.com/babelforce/rtvbp/sdk/go@v0.1.1
```

```sh title="Rust"
cargo add rtvbp \
  --git https://github.com/babelforce/rtvbp \
  --tag sdk/rust/v0.1.0
```

## What is attached

Every release includes a deterministic manifest naming the exact Git tag and commit, catalog
hashes, distribution identity, and a `SHA256SUMS` file. TypeScript includes its exact npm tarball;
Rust includes Cargo's `.crate` package. A protocol release includes a deterministic archive of
catalog manifests and their conformance fixtures, vectors, and scenarios.

Go does not attach a redundant source archive: the standard module proxy is its distribution. SDKs
are libraries, so there are no platform binaries. The documentation site is published through
GitHub Pages rather than bundled into releases.

## Verify a protocol release

Download all assets and check their digests:

```sh
gh release download protocol/v1.0.0 \
  --repo babelforce/rtvbp \
  --dir rtvbp-release
(cd rtvbp-release && sha256sum -c rtvbp-protocol-v1.0.0-SHA256SUMS)
```

GitHub Actions signs build provenance for each file named by that checksum list. Verify an artifact
or manifest against the official repository:

```sh
gh attestation verify \
  rtvbp-release/rtvbp-protocol-v1.0.0.tar.gz \
  --repo babelforce/rtvbp
```

The attestation and manifest together answer two different questions: the attestation proves which
repository workflow produced a digest; the manifest ties that digest to the component version,
immutable source commit, and protocol catalogs.
