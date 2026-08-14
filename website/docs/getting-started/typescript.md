---
sidebar_position: 3
---

# TypeScript and browser SDK

The `@babelforce/rtvbp` package combines generated protocol types, validators, role adapters, typed
peers, profile metadata, and the `classic.v1` codec with hand-written Node.js and browser runtimes.
It supports Node 22+ and evergreen browsers from the same package without importing Node modules
into the browser entry point.

```sh
npm install @babelforce/rtvbp
```

Use the root export for protocol and session APIs, then select a platform entry point:

```ts
import {
  Handler,
  Session,
  babelforceV1,
  classicV1,
  profileMediaFormat,
  profiles,
} from "@babelforce/rtvbp";
import {
  BrowserAudioDevice,
  browserWebRtcTransport,
  browserWebSocketTransport,
} from "@babelforce/rtvbp/browser";
import { nodeWebSocketTransport } from "@babelforce/rtvbp/node";
```

## Browser WebSocket audio

The `rtvbp.v1` browser path carries exact L16 audio in WebSocket binary messages. The explicit
device adapter requests microphone permission, starts one `AudioContext`, owns capture and playback
AudioWorklets, continuously resamples between the device rate and 8 kHz, bounds playback, and
releases everything it created.

```ts
const device = new BrowserAudioDevice();
const format = profileMediaFormat(profiles.PROFILE_RTVBP_V1, "audio");

const handler = new Handler({
  adapter: babelforceV1.voiceAdapter(voiceHandler, voiceEvents),
  onBegin: async (context) => {
    await context.acceptAudio();
    await device.attachWebSocket(context.audio);
  },
});

const session = new Session({
  envelope: classicV1.classicV1Envelope,
  handler,
  transportFactory: browserWebSocketTransport({
    url: "wss://voice.example/rtvbp",
    audioFormat: format,
  }),
});

const running = session.run();
await session.ready;
```

Implement the generated `audioBufferClear` callback with `device.clearPlayback()`. It clears audio
already in the worklet, the session buffer, and binary frames still queued by the transport, then
returns the number of removed L16 bytes.

```ts
async audioBufferClear() {
  return { len: device.clearPlayback() };
}
```

Always close both owners. A session owns protocol and transport work; the device adapter owns only
the browser resources it created.

```ts
await session.close();
await running;
await device.close();
```

## Native browser WebRTC

The `rtvbp.webrtc.v1` browser path keeps the same generated control surface and uses the browser's
native `RTCPeerConnection` for media:

```ts
const device = new BrowserAudioDevice();
const session = new Session({
  envelope: classicV1.classicV1Envelope,
  handler: new Handler({
    adapter: babelforceV1.voiceAdapter(voiceHandler, voiceEvents),
    onBegin: async (context) => await context.acceptAudio(),
  }),
  transportFactory: browserWebRtcTransport({
    url: "wss://voice.example/rtvbp",
    audioDevice: device,
    rtcConfiguration: { iceServers },
  }),
});
```

The transport requires PCMU/8000/1, creates one send/receive audio transceiver, waits for complete
ICE gathering, and exchanges one bounded `transport.webrtc.offer` request and answer before the
session begins. SDP is never exposed to a catalog handler and should never be logged.

Browser-native WebRTC media is a track API, not a byte stream. Use `BrowserAudioDevice` for capture
and rendering and `BrowserWebRtcTransport.getStats()` for native statistics. Raw
`session.audio.read()` and `write()` fail explicitly with `media_native`; Go and Rust retain their
L16 SDK boundary while encoding and decoding PCMU in their transports.

Current v1 limits are deliberate and conspicuous: one bidirectional audio track, PCMU only,
non-trickle initial ICE, and no renegotiation, ICE restart, Opus, or transport packet-loss
concealment. Callers own `RTCConfiguration`, including every STUN/TURN URL and credential.

## Browser authentication and Origin

Native browser WebSocket cannot set an `Authorization` Upgrade header. RTVBP itself does not define
an alternative authentication scheme; choose one as deployment policy:

- same-origin cookies, which the browser applies automatically;
- a token in the caller-supplied query URL, when its logging and lifetime risks are acceptable;
- subprotocol-carried credentials supported by the accepting deployment;
- an injected non-native socket implementation that can apply headers.

For babelforce browser deployments, use the explicit deployment helper:

```ts
import { babelforceBearerSubprotocols } from "@babelforce/rtvbp/browser";

const protocols = babelforceBearerSubprotocols(
  profiles.PROFILE_RTVBP_V1,
  oauthAccessToken,
);
const transportFactory = browserWebSocketTransport({ url, protocols });
```

It sends the profile plus `bearer.<base64url(UTF-8 access token)>`, without padding. The server
decodes the bearer carrier during admission and echoes only the selected RTVBP profile. Production
browser endpoints must use `wss://`, reject untrusted `Origin` values before Upgrade, and avoid
putting tokens in logs. Browser CORS headers do not secure a WebSocket Upgrade; validate `Origin`
explicitly.

## Content Security Policy

By default, the adapter loads a short-lived `blob:` AudioWorklet module and immediately revokes its
URL. For a CSP that disallows `blob:` scripts, self-host the exact string returned by
`browserAudioWorkletModuleSource()` and pass its trusted same-origin URL as `workletModuleUrl`.

Microphone access requires a secure context (HTTPS, with localhost exceptions) and user permission.
Permission, device, autoplay, worklet, codec, negotiation, and connection failures have distinct
error codes suitable for user-facing recovery.

## Node.js headers

Node can apply Upgrade headers directly:

```ts
const transportFactory = nodeWebSocketTransport({
  url: "wss://voice.example/rtvbp",
  headers: { authorization: `Bearer ${token}` },
});
```

The Node entry point also supplies a server transport. Both generated protocol roles work in
browser clients and Node client/server sessions.
