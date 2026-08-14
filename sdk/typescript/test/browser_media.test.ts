import assert from "node:assert/strict";
import test from "node:test";

import { AsyncQueue } from "../src/async.ts";
import { AudioStream } from "../src/audio.ts";
import {
  BrowserAudioDevice,
  StatefulLinearResampler,
  babelforceBearerSubprotocols,
  browserWebRtcTransport,
} from "../src/browser.ts";
import { classicV1Envelope } from "../src/generated/zz_generated_classicv1_envelope.ts";
import {
  PROFILE_RTVBP_V1,
  PROFILE_RTVBP_WEBRTC_V1,
  SIGNALING_TRANSPORT_WEBRTC_OFFER,
} from "../src/generated/zz_generated_profiles.ts";
import type { MediaChannel, MediaFormat, MediaFrame } from "../src/transport.ts";

const audioFormat: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

class FakeMediaChannel implements MediaChannel {
  readonly id = "audio";
  readonly format = audioFormat;
  readonly inbound = new AsyncQueue<MediaFrame>(16);
  readonly outbound: MediaFrame[] = [];
  closed = false;

  async writeFrame(frame: MediaFrame): Promise<void> {
    this.outbound.push({ ...frame, data: frame.data.slice() });
  }

  async readFrame(signal?: AbortSignal): Promise<MediaFrame> {
    return await this.inbound.shift(signal);
  }

  async close(): Promise<void> {
    this.closed = true;
    this.inbound.close();
  }
}

class FakePort extends EventTarget {
  readonly sent: unknown[] = [];

  postMessage(message: unknown): void {
    this.sent.push(message);
  }

  start(): void {}

  emit(data: unknown): void {
    this.dispatchEvent(new MessageEvent("message", { data }));
  }
}

class FakeNode {
  readonly port = new FakePort();
  connections = 0;
  disconnected = false;

  connect(): FakeNode {
    this.connections += 1;
    return this;
  }

  disconnect(): void {
    this.disconnected = true;
  }
}

class FakeTrack extends EventTarget {
  readonly kind = "audio";
  readonly id = "fake-audio";
  enabled = true;
  stopped = false;

  stop(): void {
    this.stopped = true;
  }
}

class FakeStream {
  readonly track = new FakeTrack();

  getAudioTracks(): FakeTrack[] {
    return [this.track];
  }

  getTracks(): FakeTrack[] {
    return [this.track];
  }
}

class FakeAudioContext {
  readonly sampleRate = 16_000;
  readonly destination = new FakeNode();
  readonly audioWorklet = {
    modules: [] as string[],
    addModule: async (url: string): Promise<void> => { this.audioWorklet.modules.push(url); },
  };
  readonly sources: FakeNode[] = [];
  resumed = false;
  closed = false;

  createMediaStreamSource(): FakeNode {
    const node = new FakeNode();
    this.sources.push(node);
    return node;
  }

  async resume(): Promise<void> {
    this.resumed = true;
  }

  async close(): Promise<void> {
    this.closed = true;
  }
}

class FakeBrowserWebSocket extends EventTarget {
  readonly protocol = PROFILE_RTVBP_WEBRTC_V1;
  binaryType: BinaryType = "blob";
  bufferedAmount = 0;
  readyState = 0;
  offered: readonly string[] = [];
  offerMethod = "";
  offerSdp = "";

  constructor(protocols: readonly string[] | undefined) {
    super();
    this.offered = protocols ?? [];
    queueMicrotask(() => {
      this.readyState = 1;
      this.dispatchEvent(new Event("open"));
    });
  }

  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
    if (typeof data !== "string") return;
    const frame = classicV1Envelope.decode(data);
    if (frame.kind !== "request") return;
    this.offerMethod = frame.method;
    this.offerSdp = (frame.params as { readonly sdp: string }).sdp;
    const response = classicV1Envelope.encode({
      kind: "response",
      correlationId: frame.id,
      result: { sdp: "v=0\r\na=answer\r\n" },
    });
    queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data: response })));
  }

  close(): void {
    if (this.readyState === 3) return;
    this.readyState = 3;
    queueMicrotask(() => this.dispatchEvent(new Event("close")));
  }
}

class FakeTransceiver {
  readonly sender = { replaceTrack: async (): Promise<void> => {} };
  preferred: readonly RTCRtpCodec[] = [];

  setCodecPreferences(codecs: readonly RTCRtpCodec[]): void {
    this.preferred = codecs;
  }
}

class FakePeerConnection extends EventTarget {
  readonly transceiver = new FakeTransceiver();
  connectionState: RTCPeerConnectionState = "new";
  iceGatheringState: RTCIceGatheringState = "new";
  localDescription: RTCSessionDescription | null = null;
  remoteDescription: RTCSessionDescription | null = null;
  closed = false;

  addTransceiver(): FakeTransceiver {
    return this.transceiver;
  }

  async createOffer(): Promise<RTCSessionDescriptionInit> {
    return { type: "offer", sdp: "v=0\r\na=offer\r\n" };
  }

  async setLocalDescription(description: RTCLocalSessionDescriptionInit): Promise<void> {
    this.localDescription = description as RTCSessionDescription;
    this.iceGatheringState = "complete";
    this.dispatchEvent(new Event("icegatheringstatechange"));
  }

  async setRemoteDescription(description: RTCSessionDescriptionInit): Promise<void> {
    this.remoteDescription = description as RTCSessionDescription;
    this.connectionState = "connected";
    this.dispatchEvent(new Event("connectionstatechange"));
  }

  async getStats(): Promise<RTCStatsReport> {
    return new Map() as unknown as RTCStatsReport;
  }

  close(): void {
    this.closed = true;
    this.connectionState = "closed";
    this.dispatchEvent(new Event("connectionstatechange"));
  }
}

class StalledPeerConnection extends FakePeerConnection {
  override async setLocalDescription(description: RTCLocalSessionDescriptionInit): Promise<void> {
    this.localDescription = description as RTCSessionDescription;
  }
}

async function eventually(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.fail("condition was not reached");
}

test("babelforce browser auth encodes opaque OAuth tokens as a safe deployment subprotocol", () => {
  const protocols = babelforceBearerSubprotocols(
    PROFILE_RTVBP_V1,
    "opaque./+?~=token-✓",
  );
  assert.equal(protocols[0], PROFILE_RTVBP_V1);
  assert.match(protocols[1] ?? "", /^bearer\.[A-Za-z0-9_-]+$/);
  assert.equal(protocols[1]?.includes("="), false);
  const encoded = (protocols[1] ?? "").slice("bearer.".length);
  assert.equal(Buffer.from(encoded, "base64url").toString("utf8"), "opaque./+?~=token-✓");
});

test("stateful resampling is continuous across arbitrary input chunks", () => {
  const input = Float32Array.from({ length: 1_000 }, (_, index) => Math.sin(index / 11));
  const whole = new StatefulLinearResampler(48_000, 8_000).process(input);
  const chunkedResampler = new StatefulLinearResampler(48_000, 8_000);
  const parts = [
    chunkedResampler.process(input.slice(0, 317)),
    chunkedResampler.process(input.slice(317, 811)),
    chunkedResampler.process(input.slice(811)),
  ];
  const chunked = Float32Array.from(parts.flatMap((part) => [...part]));
  assert.equal(chunked.length, whole.length);
  for (let index = 0; index < whole.length; index += 1) {
    assert.ok(Math.abs((chunked[index] ?? 0) - (whole[index] ?? 0)) < 1e-6);
  }
});

test("browser audio device captures, resamples, plays, clears, and releases owned resources", async () => {
  const stream = new FakeStream();
  const context = new FakeAudioContext();
  const worklets: FakeNode[] = [];
  let revoked = "";
  const device = new BrowserAudioDevice({
    getUserMedia: async () => stream as unknown as MediaStream,
    createAudioContext: () => context as unknown as AudioContext,
    createWorkletNode: () => {
      const node = new FakeNode();
      worklets.push(node);
      return node as unknown as AudioWorkletNode;
    },
    createWorkletModuleUrl: () => "blob:public-test-worklet",
    revokeWorkletModuleUrl: (url) => { revoked = url; },
  });
  const channel = new FakeMediaChannel();
  const audio = new AudioStream();
  audio.bind(channel);
  await device.attachWebSocket(audio);

  const capture = worklets[0]!;
  const playback = worklets[1]!;
  capture.port.emit({ type: "samples", samples: Float32Array.from({ length: 320 }, (_, i) => Math.sin(i / 8)) });
  await eventually(() => channel.outbound.length === 1);
  assert.equal(channel.outbound[0]?.data.byteLength, 320);
  assert.ok(channel.outbound[0]!.data.some((byte) => byte !== 0));

  const inbound = new Uint8Array(320);
  const view = new DataView(inbound.buffer);
  for (let index = 0; index < 160; index += 1) view.setInt16(index * 2, index % 2 === 0 ? 12_000 : -12_000, true);
  await channel.inbound.push({ data: inbound });
  await eventually(() => playback.port.sent.some((message) => (message as { type?: string }).type === "samples"));
  const playbackMessage = playback.port.sent.find((message) => (message as { type?: string }).type === "samples") as {
    readonly samples: Float32Array;
  };
  assert.ok(playbackMessage.samples.some((sample) => sample !== 0));
  assert.ok(device.clearPlayback() > 0);
  assert.equal((playback.port.sent.at(-1) as { type?: string }).type, "clear");

  await device.close();
  await audio.close();
  assert.equal(stream.track.stopped, true);
  assert.equal(context.closed, true);
  assert.equal(revoked, "blob:public-test-worklet");
  assert.equal(capture.disconnected, true);
  assert.equal(playback.disconnected, true);
});

test("browser audio device reports microphone permission failures without leaking resources", async () => {
  const device = new BrowserAudioDevice({
    getUserMedia: async () => { throw new DOMException("denied", "NotAllowedError"); },
    createAudioContext: () => new FakeAudioContext() as unknown as AudioContext,
  });
  const channel = new FakeMediaChannel();
  const audio = new AudioStream();
  audio.bind(channel);
  await assert.rejects(device.attachWebSocket(audio), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "media_permission");
    return true;
  });
  await device.close();
  await audio.close();
});

test("browser WebRTC offers one PCMU audio channel through bounded classic-envelope signaling", async () => {
  let socket: FakeBrowserWebSocket | undefined;
  const peer = new FakePeerConnection();
  const factory = browserWebRtcTransport({
    url: "wss://example.invalid/rtvbp",
    createWebSocket: (_url, protocols) => {
      socket = new FakeBrowserWebSocket(protocols);
      return socket as unknown as WebSocket;
    },
    createPeerConnection: () => peer as unknown as RTCPeerConnection,
    getAudioCapabilities: () => ({
      codecs: [
        { mimeType: "audio/opus", clockRate: 48_000, channels: 2 },
        { mimeType: "audio/PCMU", clockRate: 8_000, channels: 1 },
      ],
      headerExtensions: [],
    }),
  });
  const transport = await factory(classicV1Envelope, new AbortController().signal);
  assert.deepEqual(socket?.offered, [PROFILE_RTVBP_WEBRTC_V1]);
  assert.equal(socket?.offerMethod, SIGNALING_TRANSPORT_WEBRTC_OFFER);
  assert.equal(socket?.offerSdp, "v=0\r\na=offer\r\n");
  assert.equal(peer.remoteDescription?.type, "answer");
  assert.equal(peer.transceiver.preferred.length, 1);
  assert.equal(peer.transceiver.preferred[0]?.mimeType.toLowerCase(), "audio/pcmu");

  const media = await transport.openMedia("audio", audioFormat);
  assert.equal(media.mode, "native");
  assert.deepEqual(media.format, audioFormat);
  await assert.rejects(media.writeFrame({ data: new Uint8Array(320) }), /native browser track/i);
  await transport.close();
  assert.equal(peer.closed, true);
  assert.equal(socket?.readyState, 3);
});

test("browser WebRTC cancellation stops non-trickle ICE gathering and closes both transports", async () => {
  let socket: FakeBrowserWebSocket | undefined;
  const peer = new StalledPeerConnection();
  const abort = new AbortController();
  const factory = browserWebRtcTransport({
    url: "wss://example.invalid/rtvbp",
    createWebSocket: (_url, protocols) => {
      socket = new FakeBrowserWebSocket(protocols);
      return socket as unknown as WebSocket;
    },
    createPeerConnection: () => peer as unknown as RTCPeerConnection,
    getAudioCapabilities: () => ({
      codecs: [{ mimeType: "audio/PCMU", clockRate: 8_000, channels: 1 }],
      headerExtensions: [],
    }),
  });
  const connecting = factory(classicV1Envelope, abort.signal);
  await eventually(() => peer.localDescription !== null);
  abort.abort("cancel browser negotiation");
  await assert.rejects(connecting, (error: unknown) => {
    assert.equal((error as { code?: string }).code, "aborted");
    return true;
  });
  assert.equal(peer.closed, true);
  assert.equal(socket?.readyState, 3);
});
