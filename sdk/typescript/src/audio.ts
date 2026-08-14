import { AsyncQueue } from "./async.ts";
import { SessionError, asSessionError, throwIfAborted } from "./errors.ts";
import type { MediaChannel, MediaFormat, MediaFrame } from "./transport.ts";
import { mediaFrameBytes } from "./transport.ts";

export interface ObservedMediaFrame {
  readonly direction: "in" | "out";
  readonly data: Uint8Array;
  readonly ptsMs?: number;
  readonly observedAt: number;
}

export type AudioObserver = (frame: ObservedMediaFrame) => void;

function sameFormat(left: MediaFormat, right: MediaFormat): boolean {
  return left.encoding === right.encoding
    && left.sampleRate === right.sampleRate
    && left.bitDepth === right.bitDepth
    && left.channels === right.channels
    && left.packetTimeMs === right.packetTimeMs;
}

/** Session-owned byte stream over one negotiated, fixed-size media channel. */
export class AudioStream {
  readonly #capacityBytes: number;
  readonly #observer: AudioObserver | undefined;
  readonly #onError: ((error: SessionError) => void) | undefined;
  readonly #abort = new AbortController();
  #channel: MediaChannel | undefined;
  #format: MediaFormat | undefined;
  #inbound: AsyncQueue<MediaFrame> | undefined;
  #current: Uint8Array | undefined;
  #currentOffset = 0;
  #partial = new Uint8Array(0);
  #writeTail: Promise<void> = Promise.resolve();
  #readTail: Promise<void> = Promise.resolve();
  #reader: Promise<void> | undefined;
  #closed = false;

  constructor(
    capacityBytes = 1024 * 1024,
    observer?: AudioObserver,
    onError?: (error: SessionError) => void,
  ) {
    if (!Number.isSafeInteger(capacityBytes) || capacityBytes <= 0) {
      throw new SessionError("configuration", "audio buffer capacity must be positive");
    }
    this.#capacityBytes = capacityBytes;
    this.#observer = observer;
    this.#onError = onError;
  }

  get format(): MediaFormat | undefined {
    return this.#format;
  }

  get bufferedInboundBytes(): number {
    const current = this.#current === undefined ? 0 : this.#current.byteLength - this.#currentOffset;
    if (this.#format === undefined || this.#inbound === undefined) return current;
    return current + this.#inbound.length * mediaFrameBytes(this.#format);
  }

  /** Bind exactly once. Repeating the same channel is harmless; format changes fail closed. */
  bind(channel: MediaChannel): void {
    if (this.#closed) throw new SessionError("closed", "audio stream is closed");
    mediaFrameBytes(channel.format);
    if (this.#format !== undefined && !sameFormat(this.#format, channel.format)) {
      throw new SessionError("media_format", "audio format is already negotiated");
    }
    if (this.#channel !== undefined) {
      if (this.#channel !== channel) throw new SessionError("media_duplicate", "audio is already bound");
      return;
    }
    this.#channel = channel;
    this.#format = { ...channel.format };
    const frames = Math.max(1, Math.floor(this.#capacityBytes / mediaFrameBytes(channel.format)));
    this.#inbound = new AsyncQueue<MediaFrame>(frames);
    this.#reader = this.#readFrames(channel, this.#inbound).catch((error: unknown) => {
      const failure = asSessionError(error, "media_read");
      this.#onError?.(failure);
      throw failure;
    });
    void this.#reader.catch(() => {});
  }

  async write(data: Uint8Array, signal?: AbortSignal): Promise<number> {
    throwIfAborted(signal);
    if (data.byteLength === 0) return 0;
    const copy = data.slice();
    const previous = this.#writeTail;
    let release: () => void = () => {};
    this.#writeTail = new Promise<void>((resolve) => { release = resolve; });
    await previous;
    try {
      throwIfAborted(signal);
      const channel = this.#requireChannel();
      const frameBytes = mediaFrameBytes(channel.format);
      const combined = new Uint8Array(this.#partial.byteLength + copy.byteLength);
      combined.set(this.#partial);
      combined.set(copy, this.#partial.byteLength);
      let offset = 0;
      while (combined.byteLength - offset >= frameBytes) {
        throwIfAborted(signal);
        const frame = combined.slice(offset, offset + frameBytes);
        const ptsMs = Date.now();
        await channel.writeFrame({ data: frame, ptsMs }, signal);
        this.#observe({ direction: "out", data: frame, ptsMs, observedAt: Date.now() });
        offset += frameBytes;
      }
      this.#partial = combined.slice(offset);
      return copy.byteLength;
    } finally {
      release();
    }
  }

  async read(maxBytes: number, signal?: AbortSignal): Promise<Uint8Array> {
    if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
      throw new SessionError("configuration", "read size must be positive");
    }
    throwIfAborted(signal);
    const previous = this.#readTail;
    let release: () => void = () => {};
    this.#readTail = new Promise<void>((resolve) => { release = resolve; });
    await previous;
    try {
      throwIfAborted(signal);
      const inbound = this.#requireInbound();
      if (this.#current === undefined || this.#currentOffset === this.#current.byteLength) {
        this.#current = (await inbound.shift(signal)).data;
        this.#currentOffset = 0;
      }
      const end = Math.min(this.#currentOffset + maxBytes, this.#current.byteLength);
      const result = this.#current.slice(this.#currentOffset, end);
      this.#currentOffset = end;
      return result;
    } finally {
      release();
    }
  }

  clear(): number {
    const cleared = this.bufferedInboundBytes;
    this.#current = undefined;
    this.#currentOffset = 0;
    this.#inbound?.clear();
    return cleared;
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#abort.abort(new SessionError("closed", "audio stream is closed"));
    this.#inbound?.close();
    await this.#channel?.close();
    await this.#reader?.catch(() => {});
    this.#partial = new Uint8Array(0);
  }

  async #readFrames(channel: MediaChannel, inbound: AsyncQueue<MediaFrame>): Promise<void> {
    try {
      while (!this.#abort.signal.aborted) {
        const frame = await channel.readFrame(this.#abort.signal);
        if (frame.data.byteLength !== mediaFrameBytes(channel.format)) {
          throw new SessionError("media_frame", "received media frame has the wrong byte length");
        }
        const copy = {
          data: frame.data.slice(),
          ...(frame.ptsMs === undefined ? {} : { ptsMs: frame.ptsMs }),
        };
        this.#observe({
          direction: "in",
          data: copy.data,
          ...(copy.ptsMs === undefined ? {} : { ptsMs: copy.ptsMs }),
          observedAt: Date.now(),
        });
        await inbound.push(copy, this.#abort.signal);
      }
    } catch (error) {
      if (!this.#abort.signal.aborted) {
        inbound.close();
        throw asSessionError(error, "media_read");
      }
    }
  }

  #observe(frame: ObservedMediaFrame): void {
    this.#observer?.({ ...frame, data: frame.data.slice() });
  }

  #requireChannel(): MediaChannel {
    if (this.#closed) throw new SessionError("closed", "audio stream is closed");
    if (this.#channel === undefined) throw new SessionError("media_unbound", "audio is not negotiated");
    return this.#channel;
  }

  #requireInbound(): AsyncQueue<MediaFrame> {
    this.#requireChannel();
    if (this.#inbound === undefined) throw new SessionError("media_unbound", "audio is not negotiated");
    return this.#inbound;
  }
}
