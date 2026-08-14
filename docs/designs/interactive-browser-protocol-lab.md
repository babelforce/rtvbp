# Interactive browser protocol lab

**Status:** accepted · **Story:** R-40 · **Parent:** [M2 browser parity](m2-browser-parity.md)

## Decision

Publish a first-class `/try` route in the Docusaurus site and link it from a prominent landing-page
**Try it out** call to action. The route is a glass-box phone: controls and audio state remain visible
beside the exact RTVBP lifecycle, control frames, generated scenarios, and media statistics.

The default simulation is fully local. It creates two `Session` instances from the published
TypeScript SDK, joins them with `MemoryTransport.pair()`, dispatches through generated
`babelforce.v1` role adapters and typed peers, and observes the bytes emitted by the generated
`classic.v1` envelope codec. Generated conformance scenario JSON is imported from `conformance/` at
site build time and replayed step by step. The UI never contains a second catalog or envelope model.

Media proof is also local. A user gesture starts one `AudioContext`, deterministic oscillator-backed
tracks, and two looped-back native `RTCPeerConnection` instances. Remote tracks are rendered through
the same audio context. Browser `getStats()` supplies codec, ICE/connection state, selected pair,
bitrate, RTT, jitter, and packet loss; missing browser fields render as unavailable rather than being
invented. WebSocket-profile simulation uses deterministic L16 frames over the SDK memory media
channel and labels WebRTC-only statistics as not applicable.

## Trust boundary

Simulation mode makes no network requests after the static site has loaded, asks for no account or
microphone, and uses no cookies, credentials, or telemetry. SDP and ICE candidates are deliberately
summarized and never put in the visible timeline because they may contain network addresses. Raw
payload display is off by default and is only a local rendering toggle.

Live mode is an explicitly advanced path. The visitor supplies a `wss://` endpoint, optional bearer
credential, and optional ICE configuration at runtime. These values remain in component memory,
are never persisted, placed in URLs, copied into timeline payloads, logged, or submitted anywhere
except to the selected endpoint by the SDK transport. Starting live mode is the consent boundary for
microphone access. The site provides no endpoint, account, TURN service, or credential.

## Ownership and cleanup

One lab controller owns every timer, `Session`, transport, `AudioContext`, oscillator, media track,
and peer connection created for a run. Hangup, failure, navigation, and React unmount all call the
same idempotent cleanup path. Repeated call/hangup cycles create fresh resources. Node-only SDK
exports never enter the website bundle.

## Static-site behavior

The page has useful explanatory content before hydration and the production build remains a static
artifact. Interactive controls are disabled until the client mounts. If Web Audio or WebRTC is
unavailable, the protocol simulation still runs and reports the unavailable media capability
without hiding the limitation.
