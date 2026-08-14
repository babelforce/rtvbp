import { AsyncQueue, withTimeout } from "./async.ts";
import { SessionError, aborted, throwIfAborted } from "./errors.ts";
import { HEADERLESS_PROFILE } from "./generated/zz_generated_profiles.ts";
import type {
  ControlChannel,
  KeepalivePolicy,
  MediaChannel,
  MediaFormat,
  MediaFrame,
  ReceivedControl,
  Transport,
} from "./transport.ts";
import { mediaFrameBytes } from "./transport.ts";

export type WebSocketMessage =
  | { readonly kind: "text"; readonly data: string }
  | { readonly kind: "binary"; readonly data: Uint8Array };

/** Minimal injected socket seam implemented by browser and Node entry points. */
export interface WebSocketPlatformSocket {
  readonly protocol: string;
  sendText(data: string): Promise<void>;
  sendBinary(data: Uint8Array): Promise<void>;
  close(): Promise<void>;
  onMessage(listener: (message: WebSocketMessage) => void): () => void;
  onClose(listener: (error?: unknown) => void): () => void;
  ping?(data: string): Promise<void>;
  onPong?(listener: (data: string) => void): () => void;
}

export interface WebSocketTransportConfig {
  readonly socket: WebSocketPlatformSocket;
  readonly audioFormat?: MediaFormat;
  readonly inboundCapacity?: number;
}

class SocketControl implements ControlChannel {
  readonly #transport: WebSocketTransport;

  constructor(transport: WebSocketTransport) {
    this.#transport = transport;
  }

  async send(data: string, signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    await this.#transport.sendText(data);
  }

  async receive(signal?: AbortSignal): Promise<ReceivedControl> {
    return await this.#transport.receiveControl(signal);
  }
}

class SocketMedia implements MediaChannel {
  readonly id = "audio";
  readonly #transport: WebSocketTransport;
  #format: MediaFormat | undefined;
  #closed = false;

  constructor(transport: WebSocketTransport, format?: MediaFormat) {
    this.#transport = transport;
    if (format !== undefined) this.configure(format);
  }

  get format(): MediaFormat {
    if (this.#format === undefined) {
      throw new SessionError("media_unbound", "WebSocket audio format is not configured");
    }
    return this.#format;
  }

  configure(format: MediaFormat): void {
    mediaFrameBytes(format);
    if (this.#format !== undefined) {
      const current = this.#format;
      if (
        current.encoding !== format.encoding
        || current.sampleRate !== format.sampleRate
        || current.bitDepth !== format.bitDepth
        || current.channels !== format.channels
        || current.packetTimeMs !== format.packetTimeMs
      ) {
        throw new SessionError("media_format", "WebSocket audio format is already configured");
      }
      return;
    }
    this.#format = { ...format };
  }

  async writeFrame(frame: MediaFrame, signal?: AbortSignal): Promise<void> {
    if (this.#closed) throw new SessionError("closed", "media channel is closed");
    throwIfAborted(signal);
    if (frame.data.byteLength !== mediaFrameBytes(this.format)) {
      throw new SessionError("media_frame", "media frame has the wrong byte length");
    }
    await this.#transport.sendBinary(frame.data);
  }

  async readFrame(signal?: AbortSignal): Promise<MediaFrame> {
    if (this.#closed) throw new SessionError("closed", "media channel is closed");
    const data = await this.#transport.receiveBinary(signal);
    if (data.byteLength !== mediaFrameBytes(this.format)) {
      throw new SessionError("media_frame", "received media frame has the wrong byte length");
    }
    return { data };
  }

  async close(): Promise<void> {
    this.#closed = true;
  }
}

/** Text control plus a static binary L16 audio channel over one WebSocket. */
export class WebSocketTransport implements Transport {
  readonly control: ControlChannel;
  readonly supportsKeepalive: boolean;
  readonly wireSubprotocol: string;
  readonly profile: string;
  readonly #socket: WebSocketPlatformSocket;
  readonly #controls: AsyncQueue<ReceivedControl>;
  readonly #binary: AsyncQueue<Uint8Array>;
  readonly #pongs = new AsyncQueue<string>(64);
  readonly #media: SocketMedia;
  readonly #detach: (() => void)[] = [];
  #writeTail: Promise<void> = Promise.resolve();
  #closed = false;
  #closePromise: Promise<void> | undefined;

  constructor(config: WebSocketTransportConfig) {
    this.#socket = config.socket;
    this.#controls = new AsyncQueue(config.inboundCapacity ?? 256);
    this.#binary = new AsyncQueue(config.inboundCapacity ?? 256);
    this.#media = new SocketMedia(this, config.audioFormat);
    this.control = new SocketControl(this);
    this.wireSubprotocol = config.socket.protocol;
    this.profile = this.wireSubprotocol || HEADERLESS_PROFILE;
    this.supportsKeepalive = config.socket.ping !== undefined && config.socket.onPong !== undefined;
    this.#detach.push(
      config.socket.onMessage((message) => {
        const admitted = message.kind === "text"
          ? this.#controls.tryPush({ data: message.data, receivedAt: Date.now() })
          : this.#binary.tryPush(message.data.slice());
        if (!admitted) void this.#finish(new SessionError("inbound_overflow", "WebSocket inbound queue is full"));
      }),
      config.socket.onClose(() => this.#finishQueues()),
    );
    if (config.socket.onPong !== undefined) {
      this.#detach.push(config.socket.onPong((data) => { this.#pongs.tryPush(data); }));
    }
  }

  async sendText(data: string): Promise<void> {
    await this.#enqueue(async () => await this.#socket.sendText(data));
  }

  async sendBinary(data: Uint8Array): Promise<void> {
    const copy = data.slice();
    await this.#enqueue(async () => await this.#socket.sendBinary(copy));
  }

  async receiveControl(signal?: AbortSignal): Promise<ReceivedControl> {
    return await this.#controls.shift(signal);
  }

  async receiveBinary(signal?: AbortSignal): Promise<Uint8Array> {
    return (await this.#binary.shift(signal)).slice();
  }

  async openMedia(id: string, format: MediaFormat, signal?: AbortSignal): Promise<MediaChannel> {
    throwIfAborted(signal);
    if (id !== "audio") throw new SessionError("media_unsupported", `unsupported media channel '${id}'`);
    this.#media.configure(format);
    return this.#media;
  }

  async acceptMedia(signal?: AbortSignal): Promise<MediaChannel> {
    throwIfAborted(signal);
    void this.#media.format;
    return this.#media;
  }

  async monitorKeepalive(policy: KeepalivePolicy, signal: AbortSignal): Promise<void> {
    if (!this.supportsKeepalive || this.#socket.ping === undefined) {
      throw new SessionError("keepalive_unsupported", "platform cannot send native WebSocket Ping frames");
    }
    let misses = 0;
    let serial = 0;
    while (!signal.aborted) {
      await delay(policy.intervalMs, signal);
      serial += 1;
      const expected = `rtvbp:${serial}`;
      await this.#enqueue(async () => await this.#socket.ping!(expected));
      const deadline = Date.now() + policy.timeoutMs;
      let matched = false;
      while (!matched && Date.now() < deadline) {
        const waitAbort = new AbortController();
        const onAbort = (): void => waitAbort.abort(signal.reason);
        signal.addEventListener("abort", onAbort, { once: true });
        try {
          const pong = await withTimeout(
            this.#pongs.shift(waitAbort.signal),
            Math.max(1, deadline - Date.now()),
            "keepalive_timeout",
            "WebSocket Pong timed out",
            signal,
          );
          matched = pong === expected;
        } catch (error) {
          if (signal.aborted) throw aborted(signal);
          if (error instanceof SessionError && error.code === "keepalive_timeout") break;
          throw error;
        } finally {
          signal.removeEventListener("abort", onAbort);
          waitAbort.abort();
        }
      }
      misses = matched ? 0 : misses + 1;
      if (misses >= policy.maxMisses) {
        throw new SessionError("keepalive_timeout", "WebSocket keepalive timed out");
      }
    }
  }

  async close(): Promise<void> {
    if (this.#closePromise !== undefined) return await this.#closePromise;
    this.#closed = true;
    this.#closePromise = (async () => {
      await this.#writeTail;
      await this.#socket.close();
      this.#finishQueues();
    })();
    return await this.#closePromise;
  }

  async #enqueue(write: () => Promise<void>): Promise<void> {
    if (this.#closed) throw new SessionError("closed", "WebSocket transport is closed");
    const operation = this.#writeTail.then(write);
    this.#writeTail = operation.catch(async (error: unknown) => {
      await this.#finish(error);
    });
    return await operation;
  }

  async #finish(_error: unknown): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#finishQueues();
    await this.#socket.close().catch(() => {});
  }

  #finishQueues(): void {
    this.#closed = true;
    this.#controls.close();
    this.#binary.close();
    this.#pongs.close();
    for (const detach of this.#detach.splice(0)) detach();
  }
}

async function delay(milliseconds: number, signal: AbortSignal): Promise<void> {
  throwIfAborted(signal);
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    const onAbort = (): void => {
      clearTimeout(timer);
      reject(aborted(signal));
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}
