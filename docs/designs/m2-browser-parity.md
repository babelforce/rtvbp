# M2: browser parity and a spec-generated TypeScript SDK

**Status:** proposed · **Pillar:** Integration · **Stories:** R-32 … R-38

## Decision

The next major **project milestone** is browser and TypeScript parity. It is not `protocol/v2.0.0`:
there is no evidence-backed breaking payload requirement, and `babelforce.v1`, `classic.v1`,
`rtvbp.v1`, and `rtvbp.webrtc.v1` remain frozen. The coordinated release candidates are:

| Component | Candidate | Why |
|---|---|---|
| Protocol snapshot | `protocol/v1.1.0` | additive profile metadata, conformance, and TypeScript support |
| Go SDK | `sdk/go/v0.2.0` | additive generated profile surface and three-language proof |
| Rust SDK | `sdk/rust/v0.2.0` | additive generated profile surface and three-language proof |
| TypeScript SDK | `sdk/typescript/v0.1.0` / npm `@babelforce/rtvbp` | first spec-generated browser/Node release |

Versions remain independently earned. The release review may remove an untouched component from the
train; it must not manufacture a version bump for marketing symmetry. SDK `v1.0.0` waits for a
separate public-API stability decision after real consumers have used the generated surfaces.
The public npm registry returned no visible `@babelforce/rtvbp` package on 2026-08-14; organization
ownership and publication rights must still be verified before treating the name as reserved.

## Why this milestone

Go and Rust prove language-neutral generation; M2 closes the maintained browser gap.

The remaining maintained browser implementation lives outside this public repository. It manually restates a
payload subset and `classic.v1`, parses only the envelope version, correlates only initialization,
and acknowledges every otherwise unknown request. It also contains useful production evidence:
browser-safe authentication injection, PCM conversion, device-rate resampling, AudioWorklet playback,
barge-in, transcript feedback, and an end-to-end browser echo test.

M2 migrates the useful behavior while replacing wire guesses with generated types, dispatch, peers,
validation, and envelope code. That closes the last implementation explicitly named in the original
architecture audit and makes this repository authoritative for browser use too.

## Scope

### 1. Capture migration evidence and settle JavaScript semantics first

Source-pin the external browser client and classify every behavior as protocol contract, deployment
policy, application convenience, or bug. Before an emitter is accepted, decide and test:

- `int64` values beyond JavaScript's safe-integer range, without silent precision loss;
- required, optional, and required-nullable fields;
- `classic.v1` field order, discriminator precedence, permissive responses, and error quirks;
- open JSON maps, unknown events, unknown requests, and validation failures;
- browser cancellation, socket close, audio ownership, and request correlation.

The existing client is compatibility evidence, never a competing source of truth.

#### R-33 implementation contract

The TypeScript SDK uses idiomatic JavaScript `number`, not `bigint`, for catalog integers. Its
supported integer wire domain is therefore `Number.MIN_SAFE_INTEGER..=Number.MAX_SAFE_INTEGER`.
Parsing uses a lossless numeric-token parser and rejects values outside that domain before they can
be rounded. This deliberately fails closed on the part of the spec's `int64` domain JavaScript
cannot represent exactly. Generated validators still enforce each narrower schema constraint.

Protocol floats use IEEE-754 binary64 `number`. Decode accepts finite JSON numbers that the
lossless parser can convert without integer truncation, overflow, or underflow; insignificant
decimal-to-binary rounding is accepted because Go and Rust protocol floats are also binary64.
Negative zero is rejected because JavaScript encoding would silently change its spelling. Generated
field validators retain the frozen `audio.info` compatibility envelope.

Required properties are non-optional TypeScript fields. Optional properties use `?:` with
`exactOptionalPropertyTypes`, so absence is distinct from an explicit `undefined`; required-nullable
properties use `T | null`. Runtime validation makes the same three-way distinction. Open JSON values
are recursive JSON unions, never `any`; encoding rejects `undefined`, `bigint`, non-finite numbers,
class instances, sparse arrays, accessors, duplicate keys, and cycles. Generated serializers build
declared objects as ordered field entries and sort open-map keys lexically, reproducing Go's frozen
field and map ordering.

The package is ESM-first, targets ES2022, and supports evergreen browsers plus Node 22 or newer.
The browser-neutral generated surface has no framework or DOM dependency. `lossless-json` is the one
core runtime dependency; validation and dispatch are generated rather than delegated to a schema
library. Browser and Node transports are separate exports, and transport implementations are
injectable for tests and constrained runtimes.

Every asynchronous operation accepts an `AbortSignal`; cancellation settles its promise once and
removes correlation state. A session owns its transport tasks and pending requests, but never owns a
caller's microphone, `MediaStream`, `AudioContext`, or UI. Explicit browser media adapters own only
the resources they create and expose idempotent disposal. Closing a session stops SDK-created work,
settles every pending operation, and disposes an attached SDK-created adapter without guessing about
caller-owned media.

### 2. Make profiles machine-readable

Extend the spec model with declarative transport-binding and profile metadata: ids, profile names,
transport/envelope/catalog composition, negotiation tokens and defaults, reserved signaling, and
media constraints. Generate a profile manifest, SDK constants, documentation tables, and negotiation
vectors. Procedural transport behavior remains hand-written.

This removes the current duplication across Go, Rust, and prose before adding a third language. It
must reproduce all current profile names and headerless `rtvbp.v1` behavior exactly.

### 3. Generate the TypeScript protocol surface

Add `sdk/typescript/` and a TypeScript emitter for payload types, structured validators, role-specific
handler/adapter APIs, typed peers and event emitters, and the `classic.v1` codec. Generated tests must
construct and byte-round-trip every frozen fixture and consume every generated invalid vector. The
generated layer is browser/Node neutral and carries DO-NOT-EDIT banners.

### 4. Implement the hand-written runtime and deployed bindings

Implement semantic frames, one supervised session lifecycle, response fast paths, serial request/event
dispatch, timeouts, deferred and terminal replies, memory transport, and WebSocket client/server
adapters with Go/Rust-equivalent close and keepalive behavior. Support both generated roles in Node.

For browsers, ship an explicit media/device adapter rather than putting AudioContext policy into the
protocol runtime. Migrate the proven PCM/AudioWorklet path for `rtvbp.v1` and support the existing
`rtvbp.webrtc.v1` binding through native browser WebRTC. Authentication is injected by the caller;
the babelforce OAuth-over-subprotocol convention is documented as deployment policy, not universal
RTVBP wire behavior.

### 5. Prove and migrate before publishing

- Run all payload, envelope, invalid, and both-role scenarios in TypeScript.
- Exercise live WebSocket sessions in both role directions against Go and Rust.
- Exercise non-silent browser WebRTC against both Go and Rust servers with fake media in a real
  headless browser, plus one bounded real-device smoke test.
- Replace the maintained consumer's hand-written RTVBP module with the published package and retain its
  browser voice acceptance.
- Extend the release tool and workflow with npm package provenance, tarball, manifest, checksums, and
  clean external installation before the coordinated tags are cut.

## Limitation decisions

| Current limitation | M2 decision |
|---|---|
| No authoritative TypeScript/browser SDK | **Address**; this is the milestone's primary outcome. |
| Profile facts duplicated in code and prose | **Address** with spec-owned metadata and generated projections. |
| One bidirectional `audio` channel | Preserve in v1; multiple media belongs to a new binding and a demonstrated use case. |
| WebRTC PCMU only; SDK boundary L16/8 kHz/mono/20 ms | Preserve in v1; evaluate Opus and higher-rate PCM for `webrtcws.v2`, including portable codec cost. |
| Non-trickle ICE; no restart or renegotiation | Preserve in v1; design concurrent signaling and lifecycle explicitly in `webrtcws.v2`. |
| No packet-loss concealment | Measure in browser/service acceptance; implementation policy may land independently, but is not an M2 release gate. |
| QUIC and SIP absent | Defer; each is its own binding epic after three-language/profile generation is proven. |
| Legacy `rtvbp-go` still public | Finish R-16 before calling the monorepo supersession complete. |

No limitation is fixed by mutating an existing profile. A future `webrtcws.v2` must coexist with v1
and earn its scope from measured setup latency, loss, quality, or recovery evidence.

## Ordered delivery

1. **R-33 — authority and JavaScript semantics:** the fail-closed foundation and first ready story.
2. **R-34 — profile registry:** generate shared profile facts before a third SDK copies them.
3. **R-35 — TypeScript emitter:** generated payload/role/envelope surface and exact-byte proof.
4. **R-36 — runtime and WebSocket:** both roles, memory conformance, browser/Node client, Node server.
5. **R-37 — browser media and WebRTC:** device adapter, both deployed profiles, browser auth seam.
6. **R-38 — interop, migration, and release:** three-language matrix, real consumer, docs and tags.

R-16 is a release-level prerequisite and can finish in parallel. R-20 remains optional system
acceptance and does not block M2.

## Release gates

M2 ships only when:

- `babelforce.v1` and every existing profile remain byte- and behavior-identical;
- no hand-written TypeScript catalog or envelope definitions remain in any maintained consumer;
- TypeScript passes the generated conformance suite in both roles and live Go/Rust interoperability;
- browser WebSocket and WebRTC audio are non-silent, cancellation-safe, and leak-free;
- JavaScript numeric and presence semantics are explicit and fail closed outside the supported wire
  domain;
- the public package installs from a clean project and the migrated browser consumer is deployed;
- component notes, checksums, manifests, npm provenance, and protocol bundle reproduce and verify;
- the complete local and GitHub gates are green from the immutable release tags.

## Non-goals

- `babelforce.v2` or a breaking envelope;
- a universal authentication protocol;
- UI components in the SDK;
- Opus, trickle ICE, ICE restart, renegotiation, or multiple media streams in an existing v1 binding;
- QUIC and SIP implementations in the same release.

## Principal risks

- **JavaScript numbers:** `JSON.parse` loses large integer precision before validation. R-33 must
  choose a lossless representation/parser or a deliberately fail-closed supported domain.
- **Browser media ownership:** raw PCM and native WebRTC tracks are different APIs. Keep them behind
  a transport/media adapter instead of distorting generated catalog types.
- **Interop-matrix growth:** test the smallest matrix that proves both roles, both deployed profiles,
  and all three languages; do not multiply every browser/OS permutation.
- **Deployment leakage:** browser OAuth transport and Origin policy belong in deployment adapters and
  examples, not the universal protocol catalog.
