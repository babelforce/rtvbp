import { withTimeout } from "./async.ts";
import type { BrowserAudioDevice } from "./browser_audio.ts";
import {
  openBrowserWebSocket,
  type BrowserWebSocketConfig,
} from "./browser_socket.ts";
import type { EnvelopeCodec } from "./envelope.ts";
import { SessionError, aborted, throwIfAborted } from "./errors.ts";
import {
  PROFILE_RTVBP_WEBRTC_V1,
  SIGNALING_TRANSPORT_WEBRTC_OFFER,
} from "./generated/zz_generated_profiles.ts";
import { profileMediaFormat } from "./profiles.ts";
import type {
  ControlChannel,
  KeepalivePolicy,
  MediaChannel,
  MediaFormat,
  MediaFrame,
  Transport,
  TransportFactory,
} from "./transport.ts";
import { sameMediaFormat } from "./transport.ts";
import type { WebSocketTransport } from "./websocket.ts";

const MAX_SIGNAL_BYTES = 1 << 20;
const MAX_SDP_BYTES = 512 << 10;

export interface BrowserWebRtcConfig extends Omit<BrowserWebSocketConfig, "audioFormat"> {
  readonly rtcConfiguration?: RTCConfiguration;
  readonly negotiationTimeoutMs?: number;
  readonly connectionTimeoutMs?: number;
  readonly audioDevice?: BrowserAudioDevice;
  readonly createPeerConnection?: (configuration: RTCConfiguration) => RTCPeerConnection;
  readonly getAudioCapabilities?: () => RTCRtpCapabilities | null;
  readonly onRemoteTrack?: (track: MediaStreamTrack) => void | Promise<void>;
  readonly onConnectionStateChange?: (state: RTCPeerConnectionState) => void;
}

class NativeBrowserMediaChannel implements MediaChannel {
  readonly id = "audio";
  readonly mode = "native" as const;
  readonly format: MediaFormat;
  #closed = false;

  constructor(format: MediaFormat) {
    this.format = format;
  }

  async writeFrame(_frame: MediaFrame, signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    this.#requireOpen();
    throw new SessionError(
      "media_native",
      "WebRTC audio uses a native browser track; attach BrowserAudioDevice instead of writing frames",
    );
  }

  async readFrame(signal?: AbortSignal): Promise<MediaFrame> {
    throwIfAborted(signal);
    this.#requireOpen();
    throw new SessionError(
      "media_native",
      "WebRTC audio uses a native browser track; attach BrowserAudioDevice instead of reading frames",
    );
  }

  clear(): number {
    return 0;
  }

  async close(): Promise<void> {
    this.#closed = true;
  }

  #requireOpen(): void {
    if (this.#closed) throw new SessionError("closed", "browser WebRTC media channel is closed");
  }
}

/** Composite transport: classic-envelope control on WebSocket and one native browser audio track. */
export class BrowserWebRtcTransport implements Transport {
  readonly control: ControlChannel;
  readonly profile = PROFILE_RTVBP_WEBRTC_V1;
  readonly wireSubprotocol: string;
  readonly supportsKeepalive: boolean;
  readonly #base: WebSocketTransport;
  readonly #peer: RTCPeerConnection;
  readonly #transceiver: RTCRtpTransceiver;
  readonly #media: NativeBrowserMediaChannel;
  readonly #connectionTimeoutMs: number;
  readonly #device: BrowserAudioDevice | undefined;
  readonly #onRemoteTrack: BrowserWebRtcConfig["onRemoteTrack"];
  readonly #onConnectionStateChange: BrowserWebRtcConfig["onConnectionStateChange"];
  readonly #stateWaiters = new Set<() => void>();
  #claimed = false;
  #remoteTrackSeen = false;
  #failure: SessionError | undefined;
  #closing = false;
  #closed = false;
  #closePromise: Promise<void> | undefined;

  constructor(
    base: WebSocketTransport,
    peer: RTCPeerConnection,
    transceiver: RTCRtpTransceiver,
    config: BrowserWebRtcConfig,
  ) {
    this.#base = base;
    this.#peer = peer;
    this.#transceiver = transceiver;
    this.#device = config.audioDevice;
    this.#onRemoteTrack = config.onRemoteTrack;
    this.#onConnectionStateChange = config.onConnectionStateChange;
    this.#connectionTimeoutMs = config.connectionTimeoutMs ?? 15_000;
    this.#media = new NativeBrowserMediaChannel(profileMediaFormat(PROFILE_RTVBP_WEBRTC_V1, "audio"));
    this.control = base.control;
    this.wireSubprotocol = base.wireSubprotocol;
    this.supportsKeepalive = base.supportsKeepalive;
    peer.addEventListener("connectionstatechange", this.#handleConnectionState);
    peer.addEventListener("track", this.#handleTrack);
  }

  get connectionState(): RTCPeerConnectionState {
    return this.#peer.connectionState;
  }

  async replaceLocalTrack(track: MediaStreamTrack | null): Promise<void> {
    if (track !== null && track.kind !== "audio") {
      throw new SessionError("media_channel", "local WebRTC track is not audio");
    }
    await this.#transceiver.sender.replaceTrack(track);
  }

  async getStats(): Promise<RTCStatsReport> {
    return await this.#peer.getStats();
  }

  async openMedia(id: string, format: MediaFormat, signal?: AbortSignal): Promise<MediaChannel> {
    throwIfAborted(signal);
    if (id !== "audio") throw new SessionError("media_unsupported", `unsupported media channel '${id}'`);
    if (!sameMediaFormat(format, this.#media.format)) {
      throw new SessionError("media_format", "browser WebRTC requires L16/8000/16-bit/mono/20ms at the SDK boundary");
    }
    return await this.#claimAndWait(signal);
  }

  async acceptMedia(signal?: AbortSignal): Promise<MediaChannel> {
    throwIfAborted(signal);
    return await this.#claimAndWait(signal);
  }

  async monitorKeepalive(policy: KeepalivePolicy, signal: AbortSignal): Promise<void> {
    if (this.#base.monitorKeepalive === undefined) {
      throw new SessionError("keepalive_unsupported", "browser WebSocket cannot send native Ping frames");
    }
    await this.#base.monitorKeepalive(policy, signal);
  }

  async monitor(signal: AbortSignal): Promise<void> {
    while (true) {
      throwIfAborted(signal);
      if (this.#failure !== undefined) throw this.#failure;
      if (this.#closed) throw new SessionError("closed", "WebRTC peer connection closed");
      await this.#waitStateChange(signal);
    }
  }

  async close(): Promise<void> {
    if (this.#closePromise !== undefined) return await this.#closePromise;
    this.#closing = true;
    this.#closePromise = (async () => {
      this.#peer.removeEventListener("connectionstatechange", this.#handleConnectionState);
      this.#peer.removeEventListener("track", this.#handleTrack);
      this.#device?.detachRemoteTrack();
      await this.#media.close();
      this.#peer.close();
      await this.#base.close();
      this.#closed = true;
      this.#wakeStateWaiters();
    })();
    return await this.#closePromise;
  }

  async #claimAndWait(signal?: AbortSignal): Promise<MediaChannel> {
    if (this.#claimed) throw new SessionError("media_duplicate", "browser WebRTC audio is already bound");
    this.#claimed = true;
    const waitAbort = new AbortController();
    const onAbort = (): void => waitAbort.abort(signal?.reason);
    signal?.addEventListener("abort", onAbort, { once: true });
    try {
      await withTimeout(
        this.#waitConnected(waitAbort.signal),
        this.#connectionTimeoutMs,
        "webrtc_connect_timeout",
        "WebRTC audio did not connect before its deadline",
        signal,
      );
      return this.#media;
    } catch (error) {
      this.#claimed = false;
      throw error;
    } finally {
      signal?.removeEventListener("abort", onAbort);
      waitAbort.abort();
    }
  }

  async #waitConnected(signal?: AbortSignal): Promise<void> {
    while (this.#peer.connectionState !== "connected") {
      throwIfAborted(signal);
      if (this.#failure !== undefined) throw this.#failure;
      if (this.#peer.connectionState === "closed") {
        throw new SessionError("closed", "WebRTC peer connection closed before media connected");
      }
      await this.#waitStateChange(signal);
    }
  }

  async #waitStateChange(signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    await new Promise<void>((resolve, reject) => {
      const wake = (): void => {
        signal?.removeEventListener("abort", onAbort);
        resolve();
      };
      const onAbort = (): void => {
        this.#stateWaiters.delete(wake);
        reject(aborted(signal));
      };
      this.#stateWaiters.add(wake);
      signal?.addEventListener("abort", onAbort, { once: true });
    });
  }

  readonly #handleConnectionState = (): void => {
    const state = this.#peer.connectionState;
    this.#onConnectionStateChange?.(state);
    if (state === "failed") {
      this.#failure = new SessionError("webrtc_failed", "WebRTC peer connection failed");
    } else if (state === "closed") {
      this.#closed = true;
      if (!this.#closing) this.#failure = new SessionError("webrtc_closed", "WebRTC peer connection closed unexpectedly");
    }
    this.#wakeStateWaiters();
  };

  readonly #handleTrack = (event: RTCTrackEvent): void => {
    if (event.track.kind !== "audio" || this.#remoteTrackSeen) {
      this.#failure = new SessionError("media_channel", "WebRTC v1 permits exactly one remote audio track");
      this.#wakeStateWaiters();
      return;
    }
    this.#remoteTrackSeen = true;
    void (async () => {
      await this.#device?.attachRemoteTrack(event.track);
      await this.#onRemoteTrack?.(event.track);
    })().catch((error: unknown) => {
        this.#failure = error instanceof SessionError
          ? error
          : new SessionError("media_playback", error instanceof Error ? error.message : String(error), error);
        this.#wakeStateWaiters();
      });
  };

  #wakeStateWaiters(): void {
    for (const wake of this.#stateWaiters) wake();
    this.#stateWaiters.clear();
  }
}

function positiveTimeout(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new SessionError("configuration", `${name} must be a positive safe integer`);
  }
  return value;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function validateSdp(value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || bytes(value) > MAX_SDP_BYTES) {
    throw new SessionError("webrtc_sdp", "WebRTC SDP size is invalid");
  }
  return value;
}

function signalId(): string {
  if (typeof crypto === "undefined" || typeof crypto.randomUUID !== "function") {
    throw new SessionError("webrtc_signaling", "secure browser randomness is unavailable");
  }
  return crypto.randomUUID();
}

async function waitForIceGathering(peer: RTCPeerConnection, signal: AbortSignal): Promise<void> {
  throwIfAborted(signal);
  if (peer.iceGatheringState === "complete") return;
  await new Promise<void>((resolve, reject) => {
    const finish = (callback: () => void): void => {
      peer.removeEventListener("icegatheringstatechange", onState);
      signal.removeEventListener("abort", onAbort);
      callback();
    };
    const onState = (): void => {
      if (peer.iceGatheringState === "complete") finish(resolve);
    };
    const onAbort = (): void => finish(() => reject(aborted(signal)));
    peer.addEventListener("icegatheringstatechange", onState);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

async function negotiateOffer(
  transport: BrowserWebRtcTransport,
  peer: RTCPeerConnection,
  envelope: EnvelopeCodec,
  signal: AbortSignal,
): Promise<void> {
  const offer = await peer.createOffer();
  await peer.setLocalDescription(offer);
  await waitForIceGathering(peer, signal);
  const sdp = validateSdp(peer.localDescription?.sdp);
  const id = signalId();
  const encoded = envelope.encode({
    kind: "request",
    id,
    method: SIGNALING_TRANSPORT_WEBRTC_OFFER,
    params: { sdp },
  });
  if (bytes(encoded) > MAX_SIGNAL_BYTES) {
    throw new SessionError("webrtc_signaling", "encoded WebRTC offer exceeds signaling limit");
  }
  await transport.control.send(encoded, signal);
  const received = await transport.control.receive(signal);
  if (bytes(received.data) === 0 || bytes(received.data) > MAX_SIGNAL_BYTES) {
    throw new SessionError("webrtc_signaling", "WebRTC answer frame size is invalid");
  }
  const frame = envelope.decode(received.data);
  if (frame.kind !== "response" || frame.correlationId !== id) {
    throw new SessionError("webrtc_signaling", "received an unexpected WebRTC answer frame");
  }
  if (frame.error !== undefined) {
    throw new SessionError("webrtc_rejected", `WebRTC offer was rejected (${frame.error.code})`);
  }
  const result = frame.result;
  const answer = result !== null && typeof result === "object" && !Array.isArray(result)
    ? validateSdp((result as Readonly<Record<string, unknown>>).sdp)
    : validateSdp(undefined);
  await peer.setRemoteDescription({ type: "answer", sdp: answer });
}

function pcmuCodec(config: BrowserWebRtcConfig): RTCRtpCodec {
  const capabilities = config.getAudioCapabilities?.()
    ?? (typeof RTCRtpSender === "undefined" ? null : RTCRtpSender.getCapabilities("audio"));
  const codec = capabilities?.codecs.find((candidate) => (
    candidate.mimeType.toLowerCase() === "audio/pcmu"
    && candidate.clockRate === 8_000
    && (candidate.channels === undefined || candidate.channels === 1)
  ));
  if (codec === undefined) {
    throw new SessionError("webrtc_codec", "browser does not expose mandatory PCMU/8000/1 WebRTC audio");
  }
  return codec;
}

/** Browser offerer for the frozen `rtvbp.webrtc.v1` profile. */
export function browserWebRtcTransport(config: BrowserWebRtcConfig): TransportFactory {
  const protocols = config.protocols ?? [PROFILE_RTVBP_WEBRTC_V1];
  if (protocols[0] !== PROFILE_RTVBP_WEBRTC_V1) {
    throw new SessionError(
      "configuration",
      `browser WebRTC protocols must start with '${PROFILE_RTVBP_WEBRTC_V1}'`,
    );
  }
  const negotiationTimeoutMs = positiveTimeout(config.negotiationTimeoutMs ?? 15_000, "WebRTC negotiation timeout");
  positiveTimeout(config.connectionTimeoutMs ?? 15_000, "WebRTC connection timeout");
  return async (envelope, signal) => {
    const base = await openBrowserWebSocket({
      url: config.url,
      protocols,
      ...(config.connectTimeoutMs === undefined ? {} : { connectTimeoutMs: config.connectTimeoutMs }),
      ...(config.highWaterMarkBytes === undefined ? {} : { highWaterMarkBytes: config.highWaterMarkBytes }),
      ...(config.headers === undefined ? {} : { headers: config.headers }),
      ...(config.createWebSocket === undefined ? {} : { createWebSocket: config.createWebSocket }),
    }, signal);
    let peer: RTCPeerConnection | undefined;
    const negotiationAbort = new AbortController();
    const onAbort = (): void => negotiationAbort.abort(signal.reason);
    signal.addEventListener("abort", onAbort, { once: true });
    try {
      if (base.wireSubprotocol !== PROFILE_RTVBP_WEBRTC_V1) {
        throw new SessionError(
          "unsupported_subprotocol",
          `server selected '${base.wireSubprotocol}', expected '${PROFILE_RTVBP_WEBRTC_V1}'`,
        );
      }
      const createPeer = config.createPeerConnection
        ?? ((configuration: RTCConfiguration) => new RTCPeerConnection(configuration));
      peer = createPeer(config.rtcConfiguration ?? {});
      const localTrack = await config.audioDevice?.prepareWebRtc(signal);
      const transceiver = localTrack === undefined
        ? peer.addTransceiver("audio", { direction: "sendrecv" })
        : peer.addTransceiver(localTrack, { direction: "sendrecv" });
      try {
        transceiver.setCodecPreferences([pcmuCodec(config)]);
      } catch (error) {
        throw new SessionError("webrtc_codec", "browser refused the PCMU-only codec policy", error);
      }
      const transport = new BrowserWebRtcTransport(base, peer, transceiver, config);
      await withTimeout(
        negotiateOffer(transport, peer, envelope, negotiationAbort.signal),
        negotiationTimeoutMs,
        "webrtc_negotiation_timeout",
        "WebRTC offer/answer did not complete before its deadline",
        signal,
      );
      return transport;
    } catch (error) {
      negotiationAbort.abort(error);
      peer?.close();
      await base.close().catch(() => {});
      throw error;
    } finally {
      signal.removeEventListener("abort", onAbort);
      negotiationAbort.abort();
    }
  };
}
