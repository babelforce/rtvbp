---
sidebar_position: 2
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

Future profile names will identify different combinations without changing the meaning of an
existing profile.

## Reserved transport signaling

Operation methods beginning with `transport.` are reserved for envelope-independent transport
signaling, such as future WebRTC negotiation. The reservation applies across every catalog and
envelope, and catalog validation rejects such operation names. It does not reserve event names.
