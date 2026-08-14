# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) per published component
(`spec/`, `sdk/go/`, …).

## [Unreleased]

### Changed

- Removed private producer coordinates and an obsolete source-dependent capture from the public tree;
  the release gate now rejects private Git transports and confidential identifiers before commit.
- Reconciled the shipped roadmap and public limitations into M2 browser parity: a spec-generated
  TypeScript SDK, spec-owned profile metadata, browser WebSocket/WebRTC v1 media, three-language
  proof, and real-consumer migration. WebRTC v2, QUIC, and SIP remain explicitly separate epics.
- Replaced the Docusaurus starter presentation with a responsive RTVBP overview, direct Go, Rust,
  and wire-protocol paths, and current babelforce branding with source provenance, while keeping
  generated reference documentation untouched.
- Published `sdk/go/v0.1.1` with the hardened dummyphone and `sdk/rust/v0.1.0` as the first Rust
  SDK release; both tags are verified from clean external consumers.
- Moved the Docusaurus site from `docs/` to `website/` so `docs/` can hold the contributor docs and
  the track backlog; the GitHub Pages workflow now builds from `website/`.
- Made the demo client's dummyphone a stateful, race-safe telephony adapter: DTMF sequences are
  ordered, hangup is exactly once, session variables and recordings no longer panic, and
  invalid/canceled calls return errors. The Go tag workflow now also resolves its tag from a clean
  external module.
- Made published and current-Go interoperability distinguish bounded cold `go run` startup from
  the existing protocol assertions, so clean CI runners test behavior rather than compiler-cache
  warmth; Go helper processes are also reaped on every Rust-test exit path. The Rust release
  workflow can safely recover from a transient tag run by validating and publishing an existing
  immutable `sdk/rust/v*` tag through a manual dispatch.

### Added

- Added the TypeScript/browser SDK foundation with lossless numeric-token parsing, fail-closed safe
  integer semantics, strict JSON encoding, exact round-trips for all 48 frozen fixtures, and
  digest-pinned migration evidence that publishes no private source coordinates.
- Added component-scoped release changelogs and deterministic release tooling for Go manifests,
  Rust `.crate` assets, and versioned protocol/conformance bundles, with checksums, immutable-tag
  validation, semantic predecessor links, and GitHub build-provenance attestations.
- Established `protocol/v*` as the protocol snapshot release namespace and documented the release
  contents, recovery model, and verification commands; published and independently verified the
  initial `protocol/v1.0.0` manifest/conformance bundle.
- The track backlog framework: [vision](docs/vision.md), [roadmap](docs/roadmap.md), the
  [board](docs/stories/README.md), and the design records for the spec-first re-implementation.
- Frozen `babelforce.v1` golden wire fixtures captured from `rtvbp-go v0.40.0`, with a disposable
  capture tool and byte-exact regression tests.
- The `rtvbp-go v0.40.0` SDK imported under `sdk/go/` with its history preserved, plus the Rust
  specification workspace, shared catalog model, presence-aware `Nullable<T>`, and byte-exact
  `classic.v1` reference envelope codec.
- The complete typed `babelforce.v1` catalog: ten operations, nine events, roles, terminality,
  canonical examples, target-language type hints, and typed catalog validation.
- Bidirectional spec-side byte parity for all 29 frozen `babelforce.v1` fixtures: every payload and
  event passes through its concrete Rust type, every `classic.v1` envelope decodes and re-encodes
  identically, and an inventory guard prevents fixture coverage from drifting silently.
- Expanded `babelforce.v1` wire authority to 46 source-pinned fixtures: four additive Rust event
  payloads plus Go presence, error, null-result, and float variants. A separate v0.37.2 capture
  proves all 40 shapes shared with v0.40.0 byte-identical and classifies every exclusion.
- Formalized frozen v1 semantics: `session.terminate` remains voice→application, responses allow
  both or neither of result/error, error codes are an open non-zero integer space with four named
  conventions, and catalog validation reserves `transport.*` for transport signaling.
- Added the deterministic `rtvbp-spec-gen` load → validate → resolve → emit pipeline, its committed
  `babelforce.v1` catalog manifest, non-mutating drift checks, stale-output synchronization, and a
  minimal Rust CI gate that prevents spec/generated drift from merging.
- Added generated Go `babelforce.v1` payload types, method/event constants and name methods, plus
  standard-library golden tests that construct and round-trip all 36 payload/event fixtures
  byte-for-byte. CI now checks Go formatting, generated drift, and the full imported SDK test suite.
- Added the Go semantic frame, envelope, control, transport, and timed-media runtime contracts plus
  a drain-safe in-memory transport with optional duplex media. Renamed the module to
  `github.com/babelforce/rtvbp/sdk/go`, migrated workspace imports and examples, and made the Go gate
  run directly in workspace mode.
- Added a validated target-neutral envelope model and frozen fixture registry, then generated the Go
  `classic.v1` codec from that declaration. The codec byte-round-trips all ten envelope fixtures,
  preserves deployed precedence/null/error quirks, and includes data-driven malformed and semantic
  regression tests plus a synthetic second-envelope emitter proof.
- Rebuilt the Go session around semantic control frames, serial ordered dispatch, reader-path
  response completion, explicit terminal replies, deterministic lifecycle and native keepalive;
  session-owned negotiated audio now chunks by codec `PTime`, and the legacy byte transport/parser
  APIs are removed.
- Ported the WebSocket client and server to semantic text control plus static binary audio with
  subprotocol negotiation, reverse-role media, flush-on-close, validated audio/keepalive policies,
  atomic shutdown admission and race/stress coverage.
- Added the optional `webrtcws.v1` Go binding with Pion v4: reserved envelope-encoded SDP signaling
  stays on authenticated WebSocket control while timed PCMU RTP is converted to and from the
  existing L16 byte-audio API. Plain WebSocket-binary audio remains the default and is unchanged.
- Extended the existing demo client/server with selectable WebSocket or WebRTC audio, profile
  preference and secret-free ICE/TURN configuration. Added the public binding flow, codec boundary,
  deployment guidance, limitations, and race/leak-clean duplex acceptance tests.
- Added source-pinned Go `float64` boundary authority for `audio.info`, documenting and testing the
  exact supported deployed-rate compatibility envelope and representative behavior outside it.
- Expanded the frozen `babelforce.v1` authority from 46 to 48 fixtures by proving the pinned Rust
  producer omits `output.transcript.done.text` while its serializer permits present non-empty and
  empty values; generated Go now preserves all three presence states with an optional string pointer.
- Settled `transport.*` as an operation-method-only reservation, documented the frozen top-level
  `error:null` normalization exception, and refreshed the roadmap and runnable contributor gate.
- Added generated Go role interfaces, dispatch adapters, typed peer clients, event helpers,
  semantic validators, terminal dispatch and frozen per-role rejections. Replaced the derivable
  `proto/protov1` package with generated catalog code and a thin hand-written voice/telephony bridge,
  and ported both demos to the new API.
- Added deterministic generated protocol documentation: operation, event, role and classic-envelope
  reference pages now derive from the catalog schemas, comments, examples and envelope declaration.
  Replaced the stale v1 prose with WebSocket binding and profile-negotiation guides, including the
  `rtvbp.v1` compatibility default and reserved `transport.*` operation namespace.
- Restored the documentation's integration layer with generated Mermaid pages for every typed
  conformance scenario, a proven barge-in flow, separate Go SDK and wire-protocol quickstarts,
  clarified role/transport concepts, and a deployment-specific babelforce RS256 authentication
  guide backed by a tested Go validator example.
- Added spec-authored conformance scenarios and generated language-neutral payload, classic-envelope
  and multi-message vectors. A hand-written Go harness consumes the committed monorepo artifacts,
  checks exact encoded bytes, normalizes generated IDs structurally, and executes every scenario
  through the memory transport with both application and voice roles under test.
- Added live WebSocket interoperability tests against the unmodified published `rtvbp-go v0.37.2`
  module in both role directions, covering codec negotiation, binary audio both ways, DTMF, the
  legacy application pinger, headerless profile fallback and termination. Added leak-guarded demo
  wiring tests and fixed the demo endpoint, no-audio and graceful-exit paths found while running it.
- Added the typed `demo.v1` catalog and projected it through the same manifest, Go, documentation
  and conformance-vector emitters as `babelforce.v1`. A dual-profile WebSocket example routes
  `rtvbp.v1` and `rtvbp.demo.v1` to their generated handlers, with live clients proving negotiated
  and headerless-default exchanges; explicit-empty profile configuration is now idempotent.
- Proved the new Go SDK in the separately maintained `rtvbp-openai` service: the migrated OpenAI
  Realtime GA adapter now runs on Fly, and public mic/speaker calls verified duplex audio, DTMF,
  speech barge-in with `audio.buffer.clear`, and clean termination over RTVBP.
- Added `task generate` and the ordered cross-language `task check` release gate. CI regenerates all
  manifests, Go code, reference docs and vectors, rejects any diff, then tests the Go SDK and builds
  the published site; `sdk/go/v*` tags now run the same gate before creating a GitHub release.
- Added Rust as a first-class specification-generator target and introduced the standalone
  `sdk/rust` crate. Generated payloads, structured validation, role handlers/adapters, typed peers,
  event emitters and the `classic.v1` envelope now construct and byte-round-trip every frozen
  fixture, with executable role/rejection/terminal contracts and synthetic catalog/envelope proofs.
- Added the hand-written Rust Tokio runtime, bounded duplex/timed audio, memory and semantic
  WebSocket transports, and the generated-surface-only `babelforce.v1` voice/telephony bridge.
  Parity tests cover response fast paths, serial dispatch, nested requests, timeouts, deferred and
  terminal replies, keepalive, lifecycle, media binding, authentication, headerless compatibility,
  drain-safe close, callbacks, and audio observation.
- Added the Rust `rtvbp.webrtc.v1` binding on stable `webrtc` 0.14: bounded non-trickle signaling,
  one PCMU transceiver, exact L16/G.711 conversion, RTP-derived timestamps, caller-owned ICE/TURN,
  typed control, and non-silent duplex media. Current Go and Rust interoperate over WebRTC in both
  client/server directions without carrying media in WebSocket binary frames.
- Added a thin Rust consumer for every generated payload, envelope, and both-role scenario vector,
  plus live Rust interoperability in both roles against unmodified published `rtvbp-go v0.37.2`.
  The frozen peer proof covers audio negotiation, duplex audio, DTMF, legacy application ping,
  terminal close, and the headerless profile quirk.
- Added Rust quickstart and selectable WebSocket/WebRTC examples, integration, auth, ICE/TURN,
  shutdown, migration and limitations documentation, crate packaging from committed generated
  output, and an `sdk/rust/v*` release workflow that resolves the tag from a clean external project.
