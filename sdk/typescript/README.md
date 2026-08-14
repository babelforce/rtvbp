# RTVBP TypeScript SDK

`@babelforce/rtvbp` is the generated RTVBP protocol surface plus hand-written runtimes for Node.js
and evergreen browsers. The root export is platform-neutral. Import transports and browser media
only from their dedicated entry points:

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
  babelforceBearerSubprotocols,
  browserWebRtcTransport,
  browserWebSocketTransport,
} from "@babelforce/rtvbp/browser";
import { nodeWebSocketTransport } from "@babelforce/rtvbp/node";
```

Install from the public npm registry:

```sh
npm install @babelforce/rtvbp
```

## Browser audio

`BrowserAudioDevice` explicitly owns only resources it creates. It requests one microphone track,
starts an `AudioContext`, runs capture and playback through `AudioWorklet`, performs stateful
device-rate ↔ 8 kHz conversion for WebSocket audio, bounds playback, and stops its tracks and
context on `close()`.

For `rtvbp.v1`, configure the generated profile format and attach the adapter after the handler
accepts audio:

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
// In the generated audio.buffer.clear handler:
const removedBytes = device.clearPlayback();
// On application shutdown:
await session.close();
await running;
await device.close();
```

`rtvbp.v1` carries exact little-endian L16/8000/16-bit/mono/20 ms frames as WebSocket binary
messages. The adapter converts browser `Float32` audio without resetting resampler phase between
worklet chunks.

For `rtvbp.webrtc.v1`, pass the same device to the native WebRTC transport:

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

The browser owns ICE, DTLS, SRTP, capture, rendering, and PCMU. RTVBP control remains classic
envelope text on WebSocket. Signaling remains one bounded, complete, non-trickle
`transport.webrtc.offer` request and correlated answer. Raw `session.audio.read()` / `write()` are
therefore unavailable for browser-native WebRTC; use `BrowserAudioDevice` and transport statistics.

The current frozen binding permits one bidirectional audio track, PCMU/8000/1 only, no trickle ICE,
no renegotiation, and no ICE restart. Supply STUN/TURN configuration explicitly; the SDK contains no
default service or credential.

## Browser authentication

Native browser `WebSocket` cannot set an `Authorization` Upgrade header. Authentication remains a
deployment concern:

- same-origin cookies are sent by the browser;
- query parameters can be included in the caller-supplied URL when deployment policy permits;
- an injected socket implementation can apply `headers` outside a native browser;
- babelforce browser deployments can use the explicit helper below.

```ts
const protocols = babelforceBearerSubprotocols(
  profiles.PROFILE_RTVBP_V1,
  oauthAccessToken,
);
const transportFactory = browserWebSocketTransport({ url, protocols });
```

The helper emits the RTVBP profile followed by `bearer.<base64url(UTF-8 token)>`, without padding.
The accepting endpoint decodes the bearer carrier but echoes only the selected RTVBP profile. Use
`wss://`, allowlist the browser `Origin`, keep tokens out of URLs and logs where possible, and never
put credentials in application bundles.

## Content Security Policy

The default audio adapter creates a short-lived `blob:` AudioWorklet module and revokes its URL
after loading. With a stricter CSP, self-host the string returned by
`browserAudioWorkletModuleSource()` and pass its trusted same-origin URL as `workletModuleUrl`.

## Node.js

Node 22 or newer can inject Upgrade headers directly:

```ts
const transportFactory = nodeWebSocketTransport({
  url: "wss://voice.example/rtvbp",
  headers: { authorization: `Bearer ${token}` },
});
```

The Node entry point also exports `NodeWebSocketServer`; both generated roles are supported on
client and server sessions.
