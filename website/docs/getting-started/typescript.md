---
sidebar_position: 3
sidebar_label: TypeScript SDK
description: Build RTVBP peers in Node 22+ or evergreen browsers with generated contracts, WebSocket audio, and native WebRTC.
---

# TypeScript and browser SDK

The `@babelforce/rtvbp` package combines generated protocol types, validators, role adapters, typed
peers, profile metadata, and the `classic.v1` codec with hand-written Node.js and browser runtimes.
It supports Node 22+ and evergreen browsers from the same package without importing Node modules
into the browser entry point.

:::tip See the protocol before wiring an endpoint
The [browser protocol lab](/try) runs the published package locally with generated frames, duplex
audio, conformance scenarios, and WebRTC statistics. It needs no account or backend.
:::

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
  type WireJsonValue,
} from "@babelforce/rtvbp";
import {
  BrowserAudioDevice,
  BrowserWebRtcTransport,
  browserWebRtcTransport,
  browserWebSocketTransport,
} from "@babelforce/rtvbp/browser";
```

The `/browser` entry contains no Node imports. Import `/node` only in a separate Node entry point,
as shown below.

## Browser WebSocket audio

The `rtvbp.v1` browser path carries exact L16 audio in WebSocket binary messages. The explicit
device adapter requests microphone permission, starts one `AudioContext`, owns capture and playback
AudioWorklets, continuously resamples between the device rate and 8 kHz, bounds playback, and
releases everything it created.

```ts
const device = new BrowserAudioDevice();
const format = profileMediaFormat(profiles.PROFILE_RTVBP_V1, "audio");
const endpoint = "wss://voice.example/rtvbp";
const variables: Record<string, WireJsonValue> = {};

function createVoiceHandler(audioDevice: BrowserAudioDevice): babelforceV1.VoiceHandler {
  return {
    applicationMove: async () => ({}),
    audioBufferClear: async () => ({ len: audioDevice.clearPlayback() }),
    callHangup: async () => ({}),
    ping: async (context, request) => {
      const handledAt = Date.now();
      return {
        t0: request.t0,
        t1: context.receivedAt ?? handledAt,
        t2: handledAt,
        owd: 0,
        ...(request.data === undefined ? {} : { data: request.data }),
      };
    },
    recordingStart: async () => ({ id: crypto.randomUUID() }),
    recordingStop: async () => ({}),
    sessionGet: async (_context, request) => request.keys.length === 0
      ? { ...variables }
      : Object.fromEntries(
          request.keys
            .filter((key) => variables[key] !== undefined)
            .map((key) => [key, variables[key] as WireJsonValue]),
        ),
    sessionSet: async (_context, request) => {
      Object.assign(variables, request.data);
      return {};
    },
  };
}

const voiceHandler = createVoiceHandler(device);

const voiceEvents = {
  agentToolCall: async () => {},
  audioSpeechStarted: async () => {},
  inputTranscript: async () => {},
  outputTranscriptDelta: async () => {},
  outputTranscriptDone: async () => {},
} satisfies babelforceV1.VoiceEventHandler;

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
    url: endpoint,
    audioFormat: format,
  }),
});

const running = session.run();
try {
  await session.ready;
  await running;
} finally {
  await session.close().catch(() => {});
  await device.close();
}
```

The complete generated `VoiceHandler` and `VoiceEventHandler` contracts above make omissions a
compile-time error. Replace the deliberately minimal telephony behavior with your application. The
`audioBufferClear` callback uses `device.clearPlayback()`; it clears audio
already in the worklet, the session buffer, and binary frames still queued by the transport, then
returns the number of removed L16 bytes.

Always use `try`/`finally`: session setup, AudioWorklet loading, or media negotiation can fail after
the adapter has acquired resources. Start the example from a user gesture such as a call button;
some browsers will not resume an `AudioContext` from the later network-driven `onBegin` callback.
Autoplay failures are reported as `media_autoplay`.

## Native browser WebRTC

The `rtvbp.webrtc.v1` browser path keeps the same generated control surface and uses the browser's
native `RTCPeerConnection` for media:

```ts
const webRtcDevice = new BrowserAudioDevice();
const webRtcVoiceHandler = createVoiceHandler(webRtcDevice);
const webRtcSession = new Session({
  envelope: classicV1.classicV1Envelope,
  handler: new Handler({
    adapter: babelforceV1.voiceAdapter(webRtcVoiceHandler, voiceEvents),
    onBegin: async (context) => await context.acceptAudio(),
  }),
  transportFactory: browserWebRtcTransport({
    url: endpoint,
    audioDevice: webRtcDevice,
    rtcConfiguration: {
      iceServers: [{ urls: "stun:stun.example:3478" }],
    },
  }),
});

const webRtcRunning = webRtcSession.run();
try {
  await webRtcSession.ready;
  const transport = webRtcSession.transport;
  if (!(transport instanceof BrowserWebRtcTransport)) {
    throw new Error("WebRTC transport was not selected");
  }
  const report = await transport.getStats();
  await webRtcRunning;
} finally {
  await webRtcSession.close().catch(() => {});
  await webRtcDevice.close();
}
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

function authenticatedWebSocketFactory(oauthAccessToken: string) {
  return browserWebSocketTransport({
    url: endpoint,
    protocols: babelforceBearerSubprotocols(
      profiles.PROFILE_RTVBP_V1,
      oauthAccessToken,
    ),
    audioFormat: format,
  });
}

function authenticatedWebRtcFactory(
  oauthAccessToken: string,
  audioDevice: BrowserAudioDevice,
) {
  return browserWebRtcTransport({
    url: endpoint,
    protocols: babelforceBearerSubprotocols(
      profiles.PROFILE_RTVBP_WEBRTC_V1,
      oauthAccessToken,
    ),
    audioDevice,
  });
}
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
import { nodeWebSocketTransport } from "@babelforce/rtvbp/node";

function authenticatedNodeFactory(token: string) {
  return nodeWebSocketTransport({
    url: "wss://voice.example/rtvbp",
    headers: { authorization: `Bearer ${token}` },
  });
}
```

The Node entry point also supplies a server transport. Both generated protocol roles work in
browser clients and Node client/server sessions.

## Migrating an existing browser client

Remove local RTVBP payload interfaces, method/event strings, envelope parsing, request correlation,
and catch-all request acknowledgements. Replace them with generated `babelforceV1` handlers, peers,
and event emitters plus `classicV1.classicV1Envelope` and `Session`. Unknown requests then fail with
the protocol's explicit `501` behavior instead of being silently accepted.

Keep application UI state and deployment token acquisition outside the SDK. Move raw WebSocket L16
capture/playback into `BrowserAudioDevice`, or let `browserWebRtcTransport` own the native WebRTC
track binding. Use the generated `profiles` constants rather than copying subprotocol or media
facts. The maintained consumer migration is tested separately; earlier browser implementations are
compatibility evidence, never another wire source of truth.
