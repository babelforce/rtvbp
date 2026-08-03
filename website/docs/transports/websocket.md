---
sidebar_position: 1
---

# WebSocket transport binding

The deployed binding carries one RTVBP session on one WebSocket connection. Production endpoints
use TLS (`wss://`). The connecting peer initiates an HTTP Upgrade and profile negotiation follows
the rules in [Profiles and negotiation](../profiles.md).

## Authentication

The connecting peer sends a bearer credential in the Upgrade request:

```http
Authorization: Bearer <token>
```

JWT signature, issuer, audience, expiry, key distribution, and rotation checks are deployment
policy rather than catalog or envelope semantics. Servers must validate the credential before
accepting an authenticated session.

## Framing

| WebSocket message | RTVBP content |
| --- | --- |
| Text | One complete JSON control message encoded with the selected envelope |
| Binary | Audio bytes for the session's single duplex `audio` media channel |

Control and audio messages retain WebSocket ordering. Control JSON is never carried in binary
messages, and audio is never base64-encoded into control JSON.

The peers negotiate the audio codec through `session.initialize`. In `rtvbp.v1`, the deployed
format is fixed-width L16 PCM and the local binding uses 20 ms audio frames. A peer may send audio
only after initialization and codec selection complete.

## Liveness and closing

WebSocket Ping/Pong is the transport liveness mechanism. The catalog's `ping` operation is a
separate application timing measurement and is not an automatic keepalive.

For terminal operations, the response is sent and flushed before the transport closes. Normal
WebSocket close frames use the standard orderly-close handshake.
