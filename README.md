# RTVBP — Real-Time Voice Bridge Protocol

RTVBP connects a voice-owning telephony peer to an application such as an AI agent or IVR. This
repository contains the typed protocol specification, its generator, generated reference
documentation and conformance vectors, and the Go SDK.

The specification is the source of truth. Run `task generate` to regenerate every derived artifact
and `task check` to execute the same drift gate as CI.

## Get started

- [Protocol documentation](https://babelforce.github.io/rtvbp/)
- [Go SDK](sdk/go/README.md)
- [Contributor vision and principles](docs/vision.md)

```bash
go get github.com/babelforce/rtvbp/sdk/go@v0.1.0-rc.3
```
