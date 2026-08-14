import { SessionError, aborted, throwIfAborted } from "./errors.ts";
import { DEFAULT_PROFILE } from "./generated/zz_generated_profiles.ts";
import type { MediaFormat, TransportFactory } from "./transport.ts";
import {
  WebSocketTransport,
  type WebSocketMessage,
  type WebSocketPlatformSocket,
} from "./websocket.ts";

export type BrowserWebSocketFactory = (
  url: string,
  protocols: readonly string[] | undefined,
) => WebSocket;

export interface BrowserWebSocketConfig {
  readonly url: string;
  /** Undefined offers the default profile; [] deliberately performs a legacy headerless handshake. */
  readonly protocols?: readonly string[];
  readonly audioFormat?: MediaFormat;
  readonly connectTimeoutMs?: number;
  readonly highWaterMarkBytes?: number;
  readonly createWebSocket?: BrowserWebSocketFactory;
}

class BrowserSocketAdapter implements WebSocketPlatformSocket {
  readonly #socket: WebSocket;
  readonly #highWaterMarkBytes: number;
  readonly #messages = new Set<(message: WebSocketMessage) => void>();
  readonly #closed = new Set<(error?: unknown) => void>();
  #closePromise: Promise<void> | undefined;

  constructor(socket: WebSocket, highWaterMarkBytes: number) {
    this.#socket = socket;
    this.#highWaterMarkBytes = highWaterMarkBytes;
    socket.binaryType = "arraybuffer";
    socket.addEventListener("message", (event) => { void this.#handleMessage(event.data); });
    socket.addEventListener("close", () => {
      for (const listener of this.#closed) listener();
    });
    socket.addEventListener("error", () => {
      if (socket.readyState !== 3) return;
      for (const listener of this.#closed) listener(new SessionError("websocket", "WebSocket failed"));
    });
  }

  get protocol(): string {
    return this.#socket.protocol;
  }

  async sendText(data: string): Promise<void> {
    await this.#send(data);
  }

  async sendBinary(data: Uint8Array): Promise<void> {
    await this.#send(data);
  }

  onMessage(listener: (message: WebSocketMessage) => void): () => void {
    this.#messages.add(listener);
    return () => this.#messages.delete(listener);
  }

  onClose(listener: (error?: unknown) => void): () => void {
    this.#closed.add(listener);
    return () => this.#closed.delete(listener);
  }

  async close(): Promise<void> {
    if (this.#closePromise !== undefined) return await this.#closePromise;
    this.#closePromise = (async () => {
      if ((this.#socket.readyState as number) === 3) return;
      while (this.#socket.readyState === 1 && this.#socket.bufferedAmount > 0) {
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
      if (this.#socket.readyState === 3) return;
      await new Promise<void>((resolve) => {
        const onClose = (): void => resolve();
        this.#socket.addEventListener("close", onClose, { once: true });
        this.#socket.close(1000, "Closed");
      });
    })();
    return await this.#closePromise;
  }

  async #send(data: string | Uint8Array): Promise<void> {
    if (this.#socket.readyState !== 1) throw new SessionError("closed", "WebSocket is not open");
    while (this.#socket.bufferedAmount >= this.#highWaterMarkBytes) {
      await new Promise((resolve) => setTimeout(resolve, 0));
      if (this.#socket.readyState !== 1) throw new SessionError("closed", "WebSocket is not open");
    }
    this.#socket.send(typeof data === "string" ? data : data.slice().buffer as ArrayBuffer);
  }

  async #handleMessage(data: unknown): Promise<void> {
    let message: WebSocketMessage;
    if (typeof data === "string") message = { kind: "text", data };
    else if (data instanceof ArrayBuffer) message = { kind: "binary", data: new Uint8Array(data) };
    else if (ArrayBuffer.isView(data)) {
      message = {
        kind: "binary",
        data: new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice(),
      };
    } else if (data instanceof Blob) {
      message = { kind: "binary", data: new Uint8Array(await data.arrayBuffer()) };
    } else {
      return;
    }
    for (const listener of this.#messages) listener(message);
  }
}

function browserSocket(config: BrowserWebSocketConfig): WebSocket {
  const protocols = config.protocols ?? [DEFAULT_PROFILE];
  if (config.createWebSocket !== undefined) {
    return config.createWebSocket(config.url, protocols.length === 0 ? undefined : protocols);
  }
  return protocols.length === 0 ? new WebSocket(config.url) : new WebSocket(config.url, [...protocols]);
}

async function waitForOpen(socket: WebSocket, timeoutMs: number, signal: AbortSignal): Promise<void> {
  throwIfAborted(signal);
  if (socket.readyState === 1) return;
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (callback: () => void): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.removeEventListener("open", onOpen);
      socket.removeEventListener("error", onError);
      socket.removeEventListener("close", onClose);
      signal.removeEventListener("abort", onAbort);
      callback();
    };
    const onOpen = (): void => finish(resolve);
    const onError = (): void => finish(() => reject(new SessionError("websocket_connect", "WebSocket connection failed")));
    const onClose = (): void => finish(() => reject(new SessionError("websocket_connect", "WebSocket closed while connecting")));
    const onAbort = (): void => {
      socket.close();
      finish(() => reject(aborted(signal)));
    };
    const timer = setTimeout(() => {
      socket.close();
      finish(() => reject(new SessionError("websocket_connect", "WebSocket connection timed out")));
    }, timeoutMs);
    socket.addEventListener("open", onOpen, { once: true });
    socket.addEventListener("error", onError, { once: true });
    socket.addEventListener("close", onClose, { once: true });
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

/** Browser transport factory with no baked-in endpoint or authentication policy. */
export function browserWebSocketTransport(config: BrowserWebSocketConfig): TransportFactory {
  if (config.url.length === 0) throw new SessionError("configuration", "WebSocket URL is required");
  const timeoutMs = config.connectTimeoutMs ?? 10_000;
  const highWaterMarkBytes = config.highWaterMarkBytes ?? 1024 * 1024;
  return async (_envelope, signal) => {
    const socket = browserSocket(config);
    const adapter = new BrowserSocketAdapter(socket, highWaterMarkBytes);
    try {
      await waitForOpen(socket, timeoutMs, signal);
      return new WebSocketTransport({
        socket: adapter,
        ...(config.audioFormat === undefined ? {} : { audioFormat: config.audioFormat }),
      });
    } catch (error) {
      await adapter.close().catch(() => {});
      throw error;
    }
  };
}
