---
sidebar_position: 4
---

# Profiles and negotiation

An RTVBP **profile** is the complete interoperable combination of:

1. a transport binding,
2. an envelope, and
3. a payload catalog.

The separation matters: the catalog defines call-control meaning, the envelope maps semantic
requests, responses, and events onto control messages, and the transport carries control and media.

## `rtvbp.v1`

The currently deployed profile selects the [WebSocket binding](./transports/websocket.md), the
generated [`classic.v1` envelope](./reference/babelforce.v1/envelopes/classic-v1.mdx), and the
generated `babelforce.v1` catalog.

WebSocket peers negotiate profiles with `Sec-WebSocket-Protocol`:

- Clients normally offer `rtvbp.v1`.
- Servers select a mutually supported offered profile.
- An explicit offer with no supported profile is rejected.
- For compatibility with deployed peers, absence of the header means the effective profile is
  `rtvbp.v1`; the server does not echo a subprotocol the client did not offer.

The demonstration catalog uses `rtvbp.demo.v1`: WebSocket `ws.v1`, the same `classic.v1` envelope,
and the generated [`demo.v1` catalog](./reference/demo.v1/operations/demo.echo.mdx). Non-default
profiles use `rtvbp.<catalog-name>.<catalog-major>` while the transport and envelope remain the
profile's documented fixed components. New combinations receive new names rather than changing an
existing profile.

## `rtvbp.webrtc.v1`

The optional [`webrtcws.v1` binding](./transports/webrtc-websocket.md) keeps classic control on
WebSocket text messages and carries audio as PCMU over WebRTC. It uses the same `classic.v1`
envelope and frozen `babelforce.v1` catalog as `rtvbp.v1`.

It is additive. A server can support both names, but the client explicitly offers the binding it is
prepared to use. `rtvbp.v1` remains the plain WebSocket-audio default, including when the
subprotocol header is absent.

## Reserved transport signaling

Operation methods beginning with `transport.` are reserved for envelope-independent transport
signaling. The WebRTC binding uses `transport.webrtc.offer` before catalog dispatch begins. The
reservation applies across every catalog and envelope, and catalog validation rejects such
operation names. It does not reserve event names.
