# Design: WebRTC audio with WebSocket control

**Status:** accepted · **Pillar:** SDK · **Stories:** R-22, R-23, R-24

## Why

The deployed WebSocket binding multiplexes control JSON and raw audio on one connection. It is
simple and compatible, but it cannot use WebRTC's browser support, NAT traversal, media timing, or
loss handling. The next Go binding is an **addition**, not a replacement: deployments retain the
existing `rtvbp.v1` WebSocket-audio binding and may instead select a Pion WebRTC peer connection for
audio. The payload catalog, classic envelope, session runtime, application-facing byte-audio API,
and existing WebSocket behavior remain unchanged.

## Binding

The additional binding is named `webrtcws.v1` and is selected with WebSocket subprotocol
`rtvbp.webrtc.v1`. Plain WebSocket audio remains `rtvbp.v1`; callers choose which binding to offer
or serve at connection setup. One WebSocket still owns one RTVBP session:

| Concern | Carrier |
|---|---|
| RTVBP request, response, and event frames | WebSocket text messages |
| Transport negotiation | classic.v1 frames using reserved `transport.webrtc.*` operations |
| Audio | one bidirectional WebRTC audio transceiver |
| Liveness | WebSocket Ping/Pong plus WebRTC connection-state supervision |
| Session close | flush control, close the peer connection, then close WebSocket |

The offerer sends one `transport.webrtc.offer` request whose payload contains the SDP offer. The
answerer returns the SDP answer as the correlated result. Pion's non-trickle gathering mode embeds
ICE candidates in SDP, making initial negotiation one bounded request/response exchange and
preventing transport signaling from leaking into the session dispatcher. Renegotiation is outside
`webrtcws.v1`; a future binding version can add it without changing a payload catalog.

Transport signaling is encoded through the selected `Envelope`, not with ad-hoc JSON. It completes
inside the transport factory before the factory returns. Catalog handlers therefore never observe
`transport.webrtc.*`, and no generated catalog types are added: operation methods under
`transport.*` are deliberately reserved for this layer.

## Pion and codec policy

The Go implementation uses `github.com/pion/webrtc/v4`. It registers PCMU/8000/1 as the WebRTC
wire codec and creates one send/receive audio transceiver before the initial offer. PCMU is chosen
because WebRTC endpoints are required to implement G.711 and browsers can interoperate without a
custom codec.

Frozen `babelforce.v1` continues to negotiate `L16/8000/1`, and `AudioStream` continues to expose
signed 16-bit little-endian PCM bytes. The transport converts at its media boundary:

```text
AudioStream L16 little-endian -> G.711 mu-law -> RTP PCMU
RTP PCMU -> G.711 mu-law decode -> AudioStream L16 little-endian
```

That conversion is hand-written transport policy, not protocol-derived output. `OpenMedia` and
`AcceptMedia` reject formats other than L16/8000/16-bit/mono/20 ms rather than silently resampling.
Each outbound 20 ms session frame becomes one 160-byte PCMU RTP packet. Incoming frames expose a
PTS derived from the RTP clock and set `Timed=true`. Packet loss is not concealed in v1; received
packets remain ordered by Pion and gaps remain timing gaps.

## Runtime shape

`transport/webrtcws` wraps the existing semantic `transport/ws.Transport` rather than cloning its
socket, authentication, profile, queue, keepalive, and flush logic. The WebSocket package gains one
generic accepted-transport decorator seam so a server can turn an upgraded base transport into a
composite transport before starting the session. The dependency direction stays one-way:
`webrtcws -> ws -> rtvbp`; `ws` never imports Pion.

The composite transport delegates `Control`, subprotocol reporting, and native keepalive to the
base WebSocket transport. It owns the Pion `PeerConnection` and its media channel. A terminal Pion
failure closes the media read side so the existing session media supervisor fails the session.
Close is idempotent and bounded: stop media admission, close Pion, then invoke the base transport's
flush-on-close path.

Configuration exposes Pion's `webrtc.Configuration`, including STUN/TURN servers, but never embeds
credentials in examples or defaults. Host candidates make local tests deterministic. Production
integrators supply their own ICE server policy.

## Public API and proof

The package provides:

- a client `rtvbp.Option` analogous to `ws.Client`;
- a server transport decorator used from `ws.ServerConfig`;
- focused codec and signaling tests;
- a loopback end-to-end test that sends typed control and non-silent audio in both directions;
- race and leak coverage for cancellation, failed negotiation, media failure, and close;
- a runnable Go example and a public binding page with an offer/answer/media flow diagram.

The loopback test inspects the selected Pion codec and RTP-derived timing so a WebSocket-binary
fallback cannot satisfy it accidentally.

## Alternatives considered

- **WebRTC data channels for control.** This removes the stable WebSocket control path, changes
  admission/authentication behavior, and delays control until ICE completes. It is a separate
  transport, not this binding.
- **Custom RTP L16.** Easy between two Pion processes but not browser-compatible. It would meet a
  narrow SDK-to-SDK test while missing the reason for adding WebRTC.
- **Opus with an SDK-owned transcoder.** Better bandwidth and quality, but adds a native or large
  codec dependency. PCMU provides a portable browser-compatible first binding; Opus can be added as
  a later negotiated binding codec.
- **Trickle ICE in v1.** It reduces setup latency but introduces concurrent signaling and
  renegotiation state. Complete SDP gathering is deliberately simpler for the first binding.

## Acceptance / done

The epic is done when a Go client and server exchange classic control over WebSocket and timed,
bidirectional audio over Pion WebRTC; the RTP codec is PCMU while the existing session API remains
L16; authentication, profile selection, ICE configuration, failure, and close semantics are tested;
the example compiles; the public site documents the binding and its limitations; and `task check`
passes without generated drift or changes to `babelforce.v1`. Existing `rtvbp.v1` WebSocket-binary
audio remains available and its test suite stays green.

## Implementation finding

The first standalone WebRTC example duplicated the existing demo pair, so it was removed. The
existing server now offers both profiles and controls their preference order; the existing client
selects its audio transport while retaining identical protocol, telephony, DTMF, device-audio, and
shutdown behavior. This makes the additive choice visible without creating parallel examples that
would drift.
