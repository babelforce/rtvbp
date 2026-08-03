---
sidebar_position: 1
---

# Real-Time Voice Bridge Protocol

RTVBP connects a **voice peer**, which owns a live telephone call and its audio, to an
**application peer**, which listens, speaks, and controls the call. The peers exchange typed
operations and events alongside a real-time audio stream.

The payload catalog is independent from its envelope and transport. A deployed connection selects
a [profile](./profiles.md): one transport, one envelope, and one payload catalog. This lets future
bindings use WebRTC, QUIC, or SIP without redefining the call-control protocol.

## Start here

- Implementing an application? Read the generated
  [application role](./reference/babelforce.v1/roles/application.mdx).
- Implementing telephony? Read the generated
  [voice role](./reference/babelforce.v1/roles/voice.mdx).
- Connecting over WebSocket? Read the
  [WebSocket transport binding](./transports/websocket.md).
- Looking up a method or event? Open the generated `babelforce.v1` reference in the sidebar.

## Current profile

The deployed `rtvbp.v1` profile combines:

| Layer | Selection |
| --- | --- |
| Transport | WebSocket: JSON text control messages and binary audio |
| Envelope | [`classic.v1`](./reference/babelforce.v1/envelopes/classic-v1.mdx) |
| Catalog | `babelforce.v1` |

The catalog and envelope reference is generated from the same machine-readable specification that
produces SDK types and conformance fixtures. Generated reference pages carry a DO-NOT-EDIT banner.
