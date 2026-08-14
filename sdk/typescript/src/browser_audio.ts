import type { AudioStream } from "./audio.ts";
import { SessionError, asSessionError, throwIfAborted } from "./errors.ts";
import { PROFILE_RTVBP_V1 } from "./generated/zz_generated_profiles.ts";
import { profileMediaFormat } from "./profiles.ts";
import { sameMediaFormat } from "./transport.ts";

const CAPTURE_PROCESSOR = "rtvbp-capture-v1";
const PLAYBACK_PROCESSOR = "rtvbp-playback-v1";

const WORKLET_SOURCE = String.raw`
class RtvbpCapture extends AudioWorkletProcessor {
  process(inputs) {
    const samples = inputs[0] && inputs[0][0];
    if (samples && samples.length > 0) {
      const copy = samples.slice();
      this.port.postMessage({ type: "samples", samples: copy }, [copy.buffer]);
    }
    return true;
  }
}

class RtvbpPlayback extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.maxSamples = Math.max(128, options.processorOptions.maxSamples | 0);
    this.queue = [];
    this.offset = 0;
    this.queued = 0;
    this.port.onmessage = (event) => {
      const message = event.data;
      if (message && message.type === "clear") {
        this.queue = [];
        this.offset = 0;
        this.queued = 0;
        return;
      }
      if (!message || message.type !== "samples" || !(message.samples instanceof Float32Array)) return;
      let samples = message.samples;
      if (samples.length > this.maxSamples) samples = samples.slice(samples.length - this.maxSamples);
      while (this.queued + samples.length > this.maxSamples && this.queue.length > 0) {
        const first = this.queue.shift();
        const dropped = first.length - this.offset;
        this.queued -= dropped;
        this.offset = 0;
        this.port.postMessage({ type: "consumed", samples: dropped });
      }
      this.queue.push(samples);
      this.queued += samples.length;
    };
  }

  process(_inputs, outputs) {
    const output = outputs[0] && outputs[0][0];
    if (!output) return true;
    let written = 0;
    while (written < output.length && this.queue.length > 0) {
      const first = this.queue[0];
      const count = Math.min(output.length - written, first.length - this.offset);
      output.set(first.subarray(this.offset, this.offset + count), written);
      written += count;
      this.offset += count;
      this.queued -= count;
      if (this.offset === first.length) {
        this.queue.shift();
        this.offset = 0;
      }
    }
    if (written > 0) this.port.postMessage({ type: "consumed", samples: written });
    return true;
  }
}

registerProcessor("${CAPTURE_PROCESSOR}", RtvbpCapture);
registerProcessor("${PLAYBACK_PROCESSOR}", RtvbpPlayback);
`;

/** Source to self-host when a site's Content Security Policy disallows the default `blob:` module. */
export function browserAudioWorkletModuleSource(): string {
  return WORKLET_SOURCE;
}

export type BrowserAudioDeviceState = "inactive" | "starting" | "active" | "failed" | "closed";

export interface BrowserAudioStats {
  readonly capturedSdkSamples: number;
  readonly playedSdkSamples: number;
  readonly captureFrames: number;
  readonly playbackFrames: number;
  readonly bufferedPlaybackSamples: number;
  /** Sum of squared native remote-track samples observed by the rendering graph. */
  readonly remoteAudioEnergy: number;
  readonly remoteTrackAttached: boolean;
}

export type BrowserGetUserMedia = (constraints: MediaStreamConstraints) => Promise<MediaStream>;
export type BrowserAudioContextFactory = () => AudioContext;
export type BrowserAudioWorkletNodeFactory = (
  context: AudioContext,
  name: string,
  options: AudioWorkletNodeOptions,
) => AudioWorkletNode;

export interface BrowserAudioDeviceConfig {
  readonly constraints?: MediaTrackConstraints | true;
  readonly stream?: MediaStream;
  readonly audioContext?: AudioContext;
  readonly playbackBufferMs?: number;
  readonly getUserMedia?: BrowserGetUserMedia;
  readonly createAudioContext?: BrowserAudioContextFactory;
  readonly createWorkletNode?: BrowserAudioWorkletNodeFactory;
  /** Pre-hosted module URL for Content Security Policies that disallow `blob:` scripts. */
  readonly workletModuleUrl?: string;
  readonly createWorkletModuleUrl?: (source: string) => string;
  readonly revokeWorkletModuleUrl?: (url: string) => void;
  readonly createMediaStream?: (track: MediaStreamTrack) => MediaStream;
  readonly onStateChange?: (state: BrowserAudioDeviceState) => void;
  readonly onError?: (error: SessionError) => void;
  /** Raw microphone RMS in the browser device domain, clamped to 0..1 for level meters. */
  readonly onCaptureLevel?: (level: number) => void;
  /** Notification after one inbound SDK audio frame is queued for browser playback. */
  readonly onPlaybackFrame?: (sdkSamples: number) => void;
}

/** Streaming linear interpolation with phase and the boundary sample retained between chunks. */
export class StatefulLinearResampler {
  readonly #inputRate: number;
  readonly #outputRate: number;
  #nextOutputAt = 0;
  #inputOffset = 0;
  #previous: number | undefined;

  constructor(inputRate: number, outputRate: number) {
    if (!Number.isFinite(inputRate) || inputRate <= 0 || !Number.isFinite(outputRate) || outputRate <= 0) {
      throw new SessionError("media_format", "resampler rates must be positive");
    }
    this.#inputRate = inputRate;
    this.#outputRate = outputRate;
  }

  process(input: Float32Array): Float32Array {
    if (input.length === 0) return new Float32Array(0);
    const output: number[] = [];
    const start = this.#inputOffset;
    const end = start + input.length - 1;
    const step = this.#inputRate / this.#outputRate;
    while (this.#nextOutputAt <= end) {
      const lower = Math.floor(this.#nextOutputAt);
      const fraction = this.#nextOutputAt - lower;
      let first: number;
      let second: number;
      if (lower < start) {
        if (this.#previous === undefined) break;
        first = this.#previous;
        second = input[0] ?? first;
      } else {
        first = input[lower - start] ?? 0;
        if (lower + 1 > end) {
          if (fraction > Number.EPSILON) break;
          second = first;
        } else {
          second = input[lower + 1 - start] ?? first;
        }
      }
      output.push(first + (second - first) * fraction);
      this.#nextOutputAt += step;
    }
    this.#previous = input[input.length - 1];
    this.#inputOffset += input.length;
    return Float32Array.from(output);
  }

  reset(): void {
    this.#nextOutputAt = 0;
    this.#inputOffset = 0;
    this.#previous = undefined;
  }
}

function floatToL16(samples: Float32Array): Uint8Array {
  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  for (let index = 0; index < samples.length; index += 1) {
    const sample = Math.max(-1, Math.min(1, samples[index] ?? 0));
    const integer = sample < 0 ? Math.round(sample * 32_768) : Math.round(sample * 32_767);
    view.setInt16(index * 2, integer, true);
  }
  return bytes;
}

function l16ToFloat(bytes: Uint8Array): Float32Array {
  if (bytes.byteLength % 2 !== 0) throw new SessionError("media_frame", "L16 audio has an odd byte length");
  const copy = bytes.slice();
  const view = new DataView(copy.buffer);
  const samples = new Float32Array(copy.byteLength / 2);
  for (let index = 0; index < samples.length; index += 1) {
    const value = view.getInt16(index * 2, true);
    samples[index] = value < 0 ? value / 32_768 : value / 32_767;
  }
  return samples;
}

function defaultGetUserMedia(constraints: MediaStreamConstraints): Promise<MediaStream> {
  if (typeof navigator === "undefined" || navigator.mediaDevices?.getUserMedia === undefined) {
    throw new SessionError("media_unsupported", "this browser does not expose getUserMedia");
  }
  return navigator.mediaDevices.getUserMedia(constraints);
}

function defaultAudioContext(): AudioContext {
  if (typeof AudioContext === "undefined") {
    throw new SessionError("media_unsupported", "this browser does not expose AudioContext");
  }
  return new AudioContext();
}

function mediaFailure(error: unknown): SessionError {
  if (error instanceof SessionError) return error;
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError" || error.name === "SecurityError") {
      return new SessionError(
        "media_permission",
        "microphone access was denied; grant permission from a secure browser context",
        error,
      );
    }
    if (error.name === "NotFoundError" || error.name === "OverconstrainedError") {
      return new SessionError("media_device", "no microphone matches the requested constraints", error);
    }
  }
  return asSessionError(error, "media_device");
}

/** Explicit owner for browser microphone, rendering, resampling, worklets, and their teardown. */
export class BrowserAudioDevice {
  readonly #config: BrowserAudioDeviceConfig;
  readonly #abort = new AbortController();
  readonly #format = profileMediaFormat(PROFILE_RTVBP_V1, "audio");
  #state: BrowserAudioDeviceState = "inactive";
  #corePromise: Promise<void> | undefined;
  #stream: MediaStream | undefined;
  #context: AudioContext | undefined;
  #ownsStream = false;
  #ownsContext = false;
  #captureSource: AudioNode | undefined;
  #captureNode: AudioWorkletNode | undefined;
  #playbackNode: AudioWorkletNode | undefined;
  #remoteSource: MediaStreamAudioSourceNode | undefined;
  #remoteAnalyser: AnalyserNode | undefined;
  #remoteProbe: ReturnType<typeof setInterval> | undefined;
  #audio: AudioStream | undefined;
  #captureResampler: StatefulLinearResampler | undefined;
  #playbackResampler: StatefulLinearResampler | undefined;
  #captureTail: Promise<void> = Promise.resolve();
  #playbackTask: Promise<void> | undefined;
  #playbackBufferedDeviceSamples = 0;
  #playbackGeneration = 0;
  readonly #capacityMs: number;
  readonly #capacityWaiters = new Set<() => void>();
  #capturedSdkSamples = 0;
  #playedSdkSamples = 0;
  #captureFrames = 0;
  #playbackFrames = 0;
  #remoteAudioEnergy = 0;

  constructor(config: BrowserAudioDeviceConfig = {}) {
    const capacity = config.playbackBufferMs ?? 500;
    if (!Number.isSafeInteger(capacity) || capacity < 20 || capacity > 10_000) {
      throw new SessionError("configuration", "playback buffer must be between 20ms and 10s");
    }
    this.#config = config;
    this.#capacityMs = capacity;
  }

  get state(): BrowserAudioDeviceState {
    return this.#state;
  }

  get stats(): BrowserAudioStats {
    return {
      capturedSdkSamples: this.#capturedSdkSamples,
      playedSdkSamples: this.#playedSdkSamples,
      captureFrames: this.#captureFrames,
      playbackFrames: this.#playbackFrames,
      bufferedPlaybackSamples: this.#context === undefined
        ? 0
        : Math.round(this.#playbackBufferedDeviceSamples * this.#format.sampleRate / this.#context.sampleRate),
      remoteAudioEnergy: this.#remoteAudioEnergy,
      remoteTrackAttached: this.#remoteSource !== undefined,
    };
  }

  async attachWebSocket(audio: AudioStream, signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    if (this.#audio !== undefined && this.#audio !== audio) {
      throw new SessionError("media_duplicate", "browser audio device is already attached");
    }
    if (audio.format === undefined) throw new SessionError("media_unbound", "session audio is not negotiated");
    if (!sameMediaFormat(audio.format, this.#format)) {
      throw new SessionError("media_format", "browser WebSocket audio requires L16/8000/16-bit/mono/20ms");
    }
    try {
      await this.#ensureCore(signal);
      if (this.#captureNode === undefined) await this.#startWorklets(audio);
      this.#audio = audio;
      this.#setState("active");
    } catch (error) {
      const failure = mediaFailure(error);
      this.#fail(failure);
      throw failure;
    }
  }

  /** Acquire and return the one audio track a browser WebRTC transport should send. */
  async prepareWebRtc(signal?: AbortSignal): Promise<MediaStreamTrack> {
    try {
      await this.#ensureCore(signal);
      const track = this.#stream?.getAudioTracks()[0];
      if (track === undefined) throw new SessionError("media_device", "microphone stream has no audio track");
      this.#setState("active");
      return track;
    } catch (error) {
      const failure = mediaFailure(error);
      this.#fail(failure);
      throw failure;
    }
  }

  /** Render a remote native WebRTC audio track through the owned or injected AudioContext. */
  async attachRemoteTrack(track: MediaStreamTrack): Promise<void> {
    if (track.kind !== "audio") throw new SessionError("media_channel", "remote WebRTC track is not audio");
    await this.#ensureCore();
    const context = this.#requireContext();
    this.#remoteSource?.disconnect();
    const stream = this.#config.createMediaStream?.(track) ?? new MediaStream([track]);
    this.#remoteSource = context.createMediaStreamSource(stream);
    this.#remoteAnalyser = context.createAnalyser();
    this.#remoteAnalyser.fftSize = 256;
    this.#remoteSource.connect(this.#remoteAnalyser).connect(context.destination);
    const samples = new Float32Array(this.#remoteAnalyser.fftSize);
    this.#remoteProbe = setInterval(() => {
      this.#remoteAnalyser?.getFloatTimeDomainData(samples);
      let energy = 0;
      for (const sample of samples) energy += sample * sample;
      this.#remoteAudioEnergy += energy;
    }, 20);
    try {
      await context.resume();
    } catch (error) {
      throw new SessionError(
        "media_autoplay",
        "browser blocked audio playback; resume it from a user gesture",
        error,
      );
    }
  }

  detachRemoteTrack(): void {
    if (this.#remoteProbe !== undefined) clearInterval(this.#remoteProbe);
    this.#remoteProbe = undefined;
    this.#remoteSource?.disconnect();
    this.#remoteAnalyser?.disconnect();
    this.#remoteSource = undefined;
    this.#remoteAnalyser = undefined;
  }

  /** Flush SDK and AudioWorklet playback queues. Native WebRTC has no SDK-owned jitter queue. */
  clearPlayback(): number {
    this.#playbackGeneration += 1;
    const audioBytes = this.#audio?.clear() ?? 0;
    const context = this.#context;
    const queuedSdkSamples = context === undefined
      ? 0
      : Math.round(this.#playbackBufferedDeviceSamples * this.#format.sampleRate / context.sampleRate);
    this.#playbackBufferedDeviceSamples = 0;
    this.#playbackNode?.port.postMessage({ type: "clear" });
    this.#wakeCapacityWaiters();
    return audioBytes + queuedSdkSamples * 2;
  }

  async close(): Promise<void> {
    if (this.#state === "closed") return;
    this.#abort.abort(new SessionError("closed", "browser audio device is closed"));
    this.clearPlayback();
    this.#captureSource?.disconnect();
    this.#captureNode?.disconnect();
    this.#playbackNode?.disconnect();
    this.detachRemoteTrack();
    await this.#captureTail.catch(() => {});
    await this.#playbackTask?.catch(() => {});
    if (this.#ownsStream) {
      for (const track of this.#stream?.getTracks() ?? []) track.stop();
    }
    if (this.#ownsContext) await this.#context?.close().catch(() => {});
    this.#setState("closed");
  }

  async #ensureCore(signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    if (this.#state === "closed") throw new SessionError("closed", "browser audio device is closed");
    if (this.#corePromise !== undefined) return await this.#corePromise;
    this.#setState("starting");
    this.#corePromise = (async () => {
      if (this.#config.stream !== undefined) {
        this.#stream = this.#config.stream;
      } else {
        const getUserMedia = this.#config.getUserMedia ?? defaultGetUserMedia;
        this.#stream = await getUserMedia({ audio: this.#config.constraints ?? true, video: false });
        this.#ownsStream = true;
      }
      throwIfAborted(signal);
      if (this.#stream.getAudioTracks().length !== 1) {
        throw new SessionError("media_device", "microphone stream must contain exactly one audio track");
      }
      if (this.#config.audioContext !== undefined) {
        this.#context = this.#config.audioContext;
      } else {
        this.#context = (this.#config.createAudioContext ?? defaultAudioContext)();
        this.#ownsContext = true;
      }
      try {
        await this.#context.resume();
      } catch (error) {
        throw new SessionError(
          "media_autoplay",
          "browser blocked audio startup; call the adapter from a user gesture",
          error,
        );
      }
    })();
    try {
      await this.#corePromise;
    } catch (error) {
      if (this.#ownsStream) for (const track of this.#stream?.getTracks() ?? []) track.stop();
      if (this.#ownsContext) await this.#context?.close().catch(() => {});
      this.#corePromise = undefined;
      throw error;
    }
  }

  async #startWorklets(audio: AudioStream): Promise<void> {
    const context = this.#requireContext();
    if (context.audioWorklet === undefined) {
      throw new SessionError("media_worklet", "this browser does not expose AudioWorklet");
    }
    const createUrl = this.#config.createWorkletModuleUrl ?? ((source: string) => {
      if (typeof URL === "undefined" || typeof Blob === "undefined") {
        throw new SessionError("media_worklet", "cannot create the default AudioWorklet module URL");
      }
      return URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
    });
    const revokeUrl = this.#config.revokeWorkletModuleUrl ?? ((url: string) => URL.revokeObjectURL(url));
    const generatedUrl = this.#config.workletModuleUrl === undefined;
    const url = this.#config.workletModuleUrl ?? createUrl(WORKLET_SOURCE);
    if (url.length === 0) throw new SessionError("media_worklet", "AudioWorklet module URL must not be empty");
    try {
      await context.audioWorklet.addModule(url);
    } catch (error) {
      throw new SessionError(
        "media_worklet",
        "AudioWorklet module loading failed; provide workletModuleUrl-compatible CSP policy",
        error,
      );
    } finally {
      if (generatedUrl) revokeUrl(url);
    }
    const makeNode = this.#config.createWorkletNode
      ?? ((owner: AudioContext, name: string, options: AudioWorkletNodeOptions) => new AudioWorkletNode(owner, name, options));
    this.#captureNode = makeNode(context, CAPTURE_PROCESSOR, {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [1],
      channelCount: 1,
      channelCountMode: "explicit",
    });
    this.#playbackNode = makeNode(context, PLAYBACK_PROCESSOR, {
      numberOfInputs: 0,
      numberOfOutputs: 1,
      outputChannelCount: [1],
      channelCount: 1,
      channelCountMode: "explicit",
      processorOptions: {
        maxSamples: Math.ceil(context.sampleRate * this.#capacityMs / 1000),
      },
    });
    this.#captureResampler = new StatefulLinearResampler(context.sampleRate, this.#format.sampleRate);
    this.#playbackResampler = new StatefulLinearResampler(this.#format.sampleRate, context.sampleRate);
    this.#captureNode.port.addEventListener("message", (event: MessageEvent<unknown>) => {
      const message = event.data as { readonly type?: string; readonly samples?: unknown };
      if (message.type !== "samples" || !(message.samples instanceof Float32Array)) return;
      const copy = message.samples.slice();
      let energy = 0;
      for (const sample of copy) energy += sample * sample;
      this.#config.onCaptureLevel?.(Math.min(1, Math.sqrt(energy / Math.max(1, copy.length))));
      this.#captureTail = this.#captureTail.then(async () => await this.#writeCapture(copy));
      void this.#captureTail.catch((error: unknown) => this.#fail(asSessionError(error, "media_capture")));
    });
    this.#playbackNode.port.addEventListener("message", (event: MessageEvent<unknown>) => {
      const message = event.data as { readonly type?: string; readonly samples?: unknown };
      if (message.type !== "consumed" || typeof message.samples !== "number") return;
      this.#playbackBufferedDeviceSamples = Math.max(0, this.#playbackBufferedDeviceSamples - message.samples);
      this.#wakeCapacityWaiters();
    });
    this.#captureNode.port.start();
    this.#playbackNode.port.start();
    this.#captureSource = context.createMediaStreamSource(this.#stream!);
    this.#captureSource.connect(this.#captureNode).connect(context.destination);
    this.#playbackNode.connect(context.destination);
    this.#audio = audio;
    this.#playbackTask = this.#playAudio();
    void this.#playbackTask.catch((error: unknown) => {
      if (!this.#abort.signal.aborted) this.#fail(asSessionError(error, "media_playback"));
    });
  }

  async #writeCapture(samples: Float32Array): Promise<void> {
    if (this.#abort.signal.aborted) return;
    const converted = this.#captureResampler!.process(samples);
    if (converted.length === 0) return;
    await this.#audio!.write(floatToL16(converted), this.#abort.signal);
    this.#capturedSdkSamples += converted.length;
    this.#captureFrames += 1;
  }

  async #playAudio(): Promise<void> {
    while (!this.#abort.signal.aborted) {
      const generation = this.#playbackGeneration;
      const bytes = await this.#audio!.read(this.#format.sampleRate * this.#format.packetTimeMs / 1000 * 2, this.#abort.signal);
      if (generation !== this.#playbackGeneration) continue;
      const converted = this.#playbackResampler!.process(l16ToFloat(bytes));
      if (converted.length === 0) continue;
      await this.#waitForPlaybackCapacity(converted.length);
      if (generation !== this.#playbackGeneration || this.#abort.signal.aborted) continue;
      const sampleCount = converted.length;
      this.#playbackBufferedDeviceSamples += sampleCount;
      this.#playedSdkSamples += bytes.byteLength / 2;
      this.#playbackFrames += 1;
      this.#config.onPlaybackFrame?.(bytes.byteLength / 2);
      this.#playbackNode!.port.postMessage({ type: "samples", samples: converted }, [converted.buffer]);
    }
  }

  async #waitForPlaybackCapacity(nextSamples: number): Promise<void> {
    const context = this.#requireContext();
    const capacity = Math.ceil(context.sampleRate * this.#capacityMs / 1000);
    while (this.#playbackBufferedDeviceSamples + nextSamples > capacity) {
      throwIfAborted(this.#abort.signal);
      await new Promise<void>((resolve, reject) => {
        const wake = (): void => {
          this.#abort.signal.removeEventListener("abort", onAbort);
          resolve();
        };
        const onAbort = (): void => {
          this.#capacityWaiters.delete(wake);
          reject(new SessionError("aborted", "browser audio playback stopped", this.#abort.signal.reason));
        };
        this.#capacityWaiters.add(wake);
        this.#abort.signal.addEventListener("abort", onAbort, { once: true });
      });
    }
  }

  #wakeCapacityWaiters(): void {
    for (const wake of this.#capacityWaiters) wake();
    this.#capacityWaiters.clear();
  }

  #requireContext(): AudioContext {
    if (this.#context === undefined) throw new SessionError("media_device", "AudioContext is not initialized");
    return this.#context;
  }

  #setState(state: BrowserAudioDeviceState): void {
    if (this.#state === state) return;
    this.#state = state;
    this.#config.onStateChange?.(state);
  }

  #fail(error: SessionError): void {
    if (this.#state !== "closed") this.#setState("failed");
    this.#config.onError?.(error);
  }
}
