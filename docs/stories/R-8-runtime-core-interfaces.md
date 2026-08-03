---
id: R-8
title: Go runtime core — frame, envelope and transport interfaces plus the memory transport
pillar: SDK
status: in-progress
priority: 9
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [sdk-go]
note: independent of R-4/R-5; defines the seams that WebRTC, QUIC and SIP must later fit
---

# Go runtime core — frame, envelope and transport interfaces plus the memory transport

## Goal
Establish the hand-written seams of the Go SDK: the semantic control frame, the pluggable envelope
interface, and a transport abstraction of one control channel plus dynamic media channels that later
bindings can satisfy without the session or the catalog noticing.

## Acceptance
- [ ] `ControlFrame` (kind, id, correlation id, method, raw payload, error, received-at) and
      `WireError` are defined; nothing above the codec sees envelope bytes.
- [ ] `Envelope`, `ControlChannel`, `MediaFormat`, `MediaFrame`, `MediaChannel`, `Transport` and
      `TransportFactory` are defined as in the design doc, with `Transport.Close` documented as
      **must flush queued control sends** before teardown.
- [ ] `TransportFactory` receives the envelope, so a composite transport can signal in the reserved
      `transport.*` namespace later.
- [ ] `OpenMedia` returns `ErrMediaUnsupported` on transports without dynamic media.
- [ ] A `memory` transport implements the full interface — control channel pair plus an optional
      in-process media channel — and is used by the runtime's own tests.
- [ ] A test asserts the flush-on-close contract: a control frame written immediately before `Close`
      is observed by the peer.
- [ ] The Go module is renamed to `github.com/babelforce/rtvbp/sdk/go`; workspace and example-module
      replacements are updated so the repository gate is the literal `go test ./...` without a
      `GOWORK=off` exception.

## Progress
- 2026-08-03: Started by separating the new semantic frame/envelope/transport seams from the legacy
  session-facing byte transport so R-9 can migrate behavior without blocking the R-8 interfaces,
  memory transport, and module-path correction.

## Notes
- Deliberately replaces today's `Transport{Write, ReadChan, Close}` plus audio-as-`io.ReadWriter`,
  which cannot express "no media", "two media streams", or timed media.
- Reserve the `transport.*` method namespace in code comments. R-13 owns publishing the reservation,
  so no catalog can claim it.
