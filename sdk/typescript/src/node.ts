import type { AddressInfo } from "node:net";
import type { IncomingMessage } from "node:http";

import WebSocket, {
  WebSocketServer,
  type RawData,
  type VerifyClientCallbackAsync,
} from "ws";

import type { EnvelopeCodec } from "./envelope.ts";
import { SessionError, aborted, throwIfAborted } from "./errors.ts";
import { DEFAULT_PROFILE, HEADERLESS_PROFILE, SERVER_PREFERENCE } from "./generated/zz_generated_profiles.ts";
import { classicV1Envelope } from "./generated/zz_generated_classicv1_envelope.ts";
import type { Handler } from "./handler.ts";
import { Session } from "./session.ts";
import type { MediaFormat, TransportFactory } from "./transport.ts";
import {
  WebSocketTransport,
  type WebSocketMessage,
  type WebSocketPlatformSocket,
} from "./websocket.ts";

export interface NodeWebSocketClientConfig {
  readonly url: string;
  /** Undefined offers the default profile; [] deliberately performs a legacy headerless handshake. */
  readonly protocols?: readonly string[];
  readonly headers?: Readonly<Record<string, string>>;
  readonly audioFormat?: MediaFormat;
  readonly connectTimeoutMs?: number;
}

class NodeSocketAdapter implements WebSocketPlatformSocket {
  readonly #socket: WebSocket;
  readonly #messages = new Set<(message: WebSocketMessage) => void>();
  readonly #closed = new Set<(error?: unknown) => void>();
  readonly #pongs = new Set<(data: string) => void>();
  #closePromise: Promise<void> | undefined;

  constructor(socket: WebSocket) {
    this.#socket = socket;
    socket.on("message", (data, isBinary) => {
      const bytes = rawBytes(data);
      const message: WebSocketMessage = isBinary
        ? { kind: "binary", data: bytes }
        : { kind: "text", data: new TextDecoder().decode(bytes) };
      for (const listener of this.#messages) listener(message);
    });
    socket.on("pong", (data) => {
      const value = new TextDecoder().decode(data);
      for (const listener of this.#pongs) listener(value);
    });
    socket.on("close", () => {
      for (const listener of this.#closed) listener();
    });
    socket.on("error", (error) => {
      for (const listener of this.#closed) listener(error);
    });
  }

  get protocol(): string {
    return this.#socket.protocol;
  }

  async sendText(data: string): Promise<void> {
    await this.#send(data, false);
  }

  async sendBinary(data: Uint8Array): Promise<void> {
    await this.#send(data, true);
  }

  async ping(data: string): Promise<void> {
    if (this.#socket.readyState !== WebSocket.OPEN) throw new SessionError("closed", "WebSocket is not open");
    await new Promise<void>((resolve, reject) => {
      this.#socket.ping(data, undefined, (error) => error == null ? resolve() : reject(error));
    });
  }

  onMessage(listener: (message: WebSocketMessage) => void): () => void {
    this.#messages.add(listener);
    return () => this.#messages.delete(listener);
  }

  onClose(listener: (error?: unknown) => void): () => void {
    this.#closed.add(listener);
    return () => this.#closed.delete(listener);
  }

  onPong(listener: (data: string) => void): () => void {
    this.#pongs.add(listener);
    return () => this.#pongs.delete(listener);
  }

  async close(): Promise<void> {
    if (this.#closePromise !== undefined) return await this.#closePromise;
    this.#closePromise = (async () => {
      if (this.#socket.readyState === WebSocket.CLOSED) return;
      await new Promise<void>((resolve) => {
        const finish = (): void => {
          this.#socket.off("close", finish);
          this.#socket.off("error", finish);
          resolve();
        };
        this.#socket.once("close", finish);
        this.#socket.once("error", finish);
        if (this.#socket.readyState === WebSocket.CONNECTING) this.#socket.terminate();
        else this.#socket.close(1000, "Closed");
      });
    })();
    return await this.#closePromise;
  }

  async #send(data: string | Uint8Array, binary: boolean): Promise<void> {
    if (this.#socket.readyState !== WebSocket.OPEN) throw new SessionError("closed", "WebSocket is not open");
    await new Promise<void>((resolve, reject) => {
      this.#socket.send(data, { binary }, (error) => error == null ? resolve() : reject(error));
    });
  }
}

function rawBytes(data: RawData): Uint8Array {
  if (Array.isArray(data)) {
    const length = data.reduce((total, part) => total + part.byteLength, 0);
    const result = new Uint8Array(length);
    let offset = 0;
    for (const part of data) {
      result.set(part, offset);
      offset += part.byteLength;
    }
    return result;
  }
  if (data instanceof ArrayBuffer) return new Uint8Array(data).slice();
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice();
}

async function waitForNodeOpen(socket: WebSocket, timeoutMs: number, signal: AbortSignal): Promise<void> {
  throwIfAborted(signal);
  if (socket.readyState === WebSocket.OPEN) return;
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (callback: () => void): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.off("open", onOpen);
      socket.off("error", onError);
      socket.off("close", onClose);
      signal.removeEventListener("abort", onAbort);
      callback();
    };
    const onOpen = (): void => finish(resolve);
    const onError = (error: Error): void => finish(() => reject(error));
    const onClose = (): void => finish(() => reject(new SessionError("websocket_connect", "WebSocket closed while connecting")));
    const onAbort = (): void => {
      socket.terminate();
      finish(() => reject(aborted(signal)));
    };
    const timer = setTimeout(() => {
      socket.terminate();
      finish(() => reject(new SessionError("websocket_connect", "WebSocket connection timed out")));
    }, timeoutMs);
    socket.once("open", onOpen);
    socket.once("error", onError);
    socket.once("close", onClose);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

/** Node client transport factory. Endpoint and authentication headers remain caller-owned. */
export function nodeWebSocketTransport(config: NodeWebSocketClientConfig): TransportFactory {
  if (config.url.length === 0) throw new SessionError("configuration", "WebSocket URL is required");
  if (Object.keys(config.headers ?? {}).some((name) => name.toLowerCase() === "sec-websocket-protocol")) {
    throw new SessionError("configuration", "configure WebSocket profiles through protocols");
  }
  const protocols = config.protocols ?? [DEFAULT_PROFILE];
  return async (_envelope, signal) => {
    const socket = protocols.length === 0
      ? new WebSocket(config.url, { headers: config.headers })
      : new WebSocket(config.url, [...protocols], { headers: config.headers });
    const adapter = new NodeSocketAdapter(socket);
    try {
      await waitForNodeOpen(socket, config.connectTimeoutMs ?? 10_000, signal);
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

export interface NodeWebSocketConnection {
  readonly request: IncomingMessage;
  readonly profile: string;
}

export interface NodeWebSocketServerConfig {
  readonly host?: string;
  readonly port?: number;
  readonly path?: string;
  readonly supportedProfiles?: readonly string[];
  readonly allowHeaderless?: boolean;
  readonly audioFormat?: MediaFormat;
  readonly envelope?: EnvelopeCodec;
  readonly authenticate?: (request: IncomingMessage) => boolean | Promise<boolean>;
  readonly createHandler: (connection: NodeWebSocketConnection) => Handler | Promise<Handler>;
  readonly onSession?: (session: Session, connection: NodeWebSocketConnection) => void;
  readonly onSessionError?: (error: unknown, connection: NodeWebSocketConnection) => void;
}

/** Standalone Node WebSocket server; handler factories may select either generated local role. */
export class NodeWebSocketServer {
  readonly #config: NodeWebSocketServerConfig;
  readonly #sessions = new Set<Session>();
  #server: WebSocketServer | undefined;

  constructor(config: NodeWebSocketServerConfig) {
    this.#config = config;
  }

  get url(): string {
    const server = this.#requireServer();
    const address = server.address();
    if (address === null || typeof address === "string") throw new SessionError("inactive", "server is not listening");
    const host = this.#config.host ?? "127.0.0.1";
    return `ws://${host}:${(address as AddressInfo).port}${this.#config.path ?? "/"}`;
  }

  async listen(): Promise<void> {
    if (this.#server !== undefined) throw new SessionError("already_run", "server has already started");
    const supported = [...(this.#config.supportedProfiles ?? SERVER_PREFERENCE)];
    const authenticate = this.#config.authenticate;
    const verifyClient: VerifyClientCallbackAsync = (info, done) => {
      if (authenticate === undefined) {
        done(true);
        return;
      }
      void Promise.resolve(authenticate(info.req)).then(
        (allowed) => done(allowed, allowed ? undefined : 401, allowed ? undefined : "Unauthorized"),
        () => done(false, 401, "Unauthorized"),
      );
    };
    const server = new WebSocketServer({
      host: this.#config.host ?? "127.0.0.1",
      port: this.#config.port ?? 0,
      path: this.#config.path ?? "/",
      verifyClient,
      handleProtocols: (offered) => {
        for (const preferred of supported) if (offered.has(preferred)) return preferred;
        return false;
      },
    });
    this.#server = server;
    server.on("connection", (socket, request) => { void this.#accept(socket, request); });
    await new Promise<void>((resolve, reject) => {
      const onListening = (): void => {
        server.off("error", onError);
        resolve();
      };
      const onError = (error: Error): void => {
        server.off("listening", onListening);
        reject(error);
      };
      server.once("listening", onListening);
      server.once("error", onError);
    });
  }

  async close(): Promise<void> {
    const server = this.#requireServer();
    await Promise.allSettled([...this.#sessions].map(async (session) => await session.close()));
    await new Promise<void>((resolve, reject) => {
      server.close((error) => error == null ? resolve() : reject(error));
    });
    this.#server = undefined;
  }

  async #accept(socket: WebSocket, request: IncomingMessage): Promise<void> {
    const profile = socket.protocol || HEADERLESS_PROFILE;
    if (socket.protocol === "" && this.#config.allowHeaderless === false) {
      socket.close(1002, "WebSocket profile required");
      return;
    }
    const connection: NodeWebSocketConnection = { request, profile };
    try {
      const handler = await this.#config.createHandler(connection);
      const transport = new WebSocketTransport({
        socket: new NodeSocketAdapter(socket),
        ...(this.#config.audioFormat === undefined ? {} : { audioFormat: this.#config.audioFormat }),
      });
      const session = new Session({
        envelope: this.#config.envelope ?? classicV1Envelope,
        handler,
        transport,
      });
      this.#sessions.add(session);
      this.#config.onSession?.(session, connection);
      void session.run().catch((error: unknown) => {
        this.#config.onSessionError?.(error, connection);
      }).finally(() => this.#sessions.delete(session));
    } catch (error) {
      this.#config.onSessionError?.(error, connection);
      socket.close(1011, "Session setup failed");
    }
  }

  #requireServer(): WebSocketServer {
    if (this.#server === undefined) throw new SessionError("inactive", "server is not listening");
    return this.#server;
  }
}
