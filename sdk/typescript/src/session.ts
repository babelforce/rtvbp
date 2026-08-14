import { AsyncQueue, withTimeout } from "./async.ts";
import { AudioStream, type AudioObserver } from "./audio.ts";
import type { ControlFrame, EnvelopeCodec } from "./envelope.ts";
import { RemoteError, SessionError, asSessionError, throwIfAborted } from "./errors.ts";
import { Handler } from "./handler.ts";
import type {
  DeferredResponse,
  HandlerContext,
  Notifier,
  NotifyOptions,
  RequestOptions,
  Requester,
  SessionState,
  WireErrorValue,
} from "./protocol.ts";
import { ProtocolHandlerError } from "./protocol.ts";
import type { KeepalivePolicy, MediaFormat, Transport, TransportFactory } from "./transport.ts";
import { validateKeepalive } from "./transport.ts";
import type { WireEncodable } from "./wire.ts";

export interface SessionConfig {
  readonly envelope: EnvelopeCodec;
  readonly handler?: Handler;
  readonly transport?: Transport;
  readonly transportFactory?: TransportFactory;
  readonly id?: string;
  readonly requestTimeoutMs?: number;
  readonly closeTimeoutMs?: number;
  readonly dispatchCapacity?: number;
  readonly audioBufferBytes?: number;
  readonly audioObserver?: AudioObserver;
  readonly keepalive?: KeepalivePolicy;
}

interface PendingRequest {
  readonly resolve: (value: unknown) => void;
  readonly reject: (reason: unknown) => void;
  readonly terminal: boolean;
  readonly signal?: AbortSignal;
  onAbort?: () => void;
  timer?: ReturnType<typeof setTimeout>;
}

interface ReplyState {
  readonly requestId: string;
  status: "unclaimed" | "deferred" | "sent";
}

function positiveTimeout(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new SessionError("configuration", `${name} must be a positive safe integer`);
  }
  return value;
}

function detachPending(pending: PendingRequest): void {
  if (pending.timer !== undefined) clearTimeout(pending.timer);
  if (pending.signal !== undefined && pending.onAbort !== undefined) {
    pending.signal.removeEventListener("abort", pending.onAbort);
  }
}

function responseError(error: unknown): WireErrorValue {
  if (error instanceof ProtocolHandlerError) return error.wire;
  return {
    code: 500,
    message: error instanceof Error ? error.message : String(error),
  };
}

class SessionContext implements HandlerContext {
  readonly #session: Session;
  readonly #reply: ReplyState | undefined;
  readonly signal: AbortSignal;
  readonly receivedAt?: number;

  constructor(session: Session, signal: AbortSignal, reply?: ReplyState, receivedAt?: number) {
    this.#session = session;
    this.signal = signal;
    this.#reply = reply;
    if (receivedAt !== undefined) this.receivedAt = receivedAt;
  }

  get sessionId(): string {
    return this.#session.id;
  }

  get state(): SessionState {
    return this.#session.state;
  }

  get audio(): AudioStream {
    return this.#session.audio;
  }

  async request(method: string, params: WireEncodable, options?: RequestOptions): Promise<unknown> {
    return await this.#session.request(method, params, options);
  }

  async notify(event: string, data: WireEncodable, options?: NotifyOptions): Promise<void> {
    await this.#session.notify(event, data, options);
  }

  async respond(result?: WireEncodable): Promise<void> {
    await this.#respond(result, undefined, false);
  }

  async respondError(error: WireErrorValue): Promise<void> {
    await this.#respond(undefined, error, false);
  }

  async respondThenClose(result?: WireEncodable): Promise<void> {
    await this.#respond(result, undefined, true);
  }

  deferResponse(): DeferredResponse {
    const reply = this.#requireReply();
    if (reply.status !== "unclaimed") {
      throw new SessionError("response_sent", "response is already claimed");
    }
    reply.status = "deferred";
    return {
      respond: async (result) => await this.respond(result),
      respondError: async (error) => await this.respondError(error),
      respondThenClose: async (result) => await this.respondThenClose(result),
    };
  }

  async openAudio(format: MediaFormat): Promise<void> {
    await this.#session.openAudio(format);
  }

  async acceptAudio(): Promise<void> {
    await this.#session.acceptAudio();
  }

  close(): void {
    this.#session.requestStop();
  }

  get replyStatus(): ReplyState["status"] | undefined {
    return this.#reply?.status;
  }

  async #respond(
    result: WireEncodable | undefined,
    error: WireErrorValue | undefined,
    closeAfter: boolean,
  ): Promise<void> {
    const reply = this.#requireReply();
    if (reply.status === "sent") throw new SessionError("response_sent", "response is already sent");
    reply.status = "sent";
    await this.#session.sendResponse(reply.requestId, result, error);
    if (closeAfter) this.#session.requestStop();
  }

  #requireReply(): ReplyState {
    if (this.#reply === undefined) {
      throw new SessionError("no_request_context", "there is no inbound request to respond to");
    }
    if (this.#session.state === "closing" || this.#session.state === "closed" || this.#session.state === "failed") {
      throw new SessionError("closed", "session is closed");
    }
    return this.#reply;
  }
}

/** Semantic RTVBP session shared by browser and Node transports. */
export class Session implements Requester, Notifier {
  readonly #config: SessionConfig;
  readonly #handler: Handler;
  readonly #abort = new AbortController();
  readonly #dispatch: AsyncQueue<Exclude<ControlFrame, { kind: "response" }>>;
  readonly #pending = new Map<string, PendingRequest>();
  readonly #requestTimeoutMs: number;
  readonly #closeTimeoutMs: number;
  readonly #readyPromise: Promise<void>;
  readonly #stoppedPromise: Promise<void>;
  readonly audio: AudioStream;
  readonly id: string;
  #readyResolve: () => void = () => {};
  #readyReject: (reason: unknown) => void = () => {};
  #stoppedResolve: () => void = () => {};
  #state: SessionState = "inactive";
  #transport: Transport | undefined;
  #runPromise: Promise<void> | undefined;
  #sequence = 0;
  #stopCause: unknown;
  #stopFailed = false;

  constructor(config: SessionConfig) {
    if (config.envelope === undefined) throw new SessionError("configuration", "envelope is required");
    if ((config.transport === undefined) === (config.transportFactory === undefined)) {
      throw new SessionError("configuration", "provide exactly one transport or transport factory");
    }
    this.#config = config;
    this.#handler = config.handler ?? new Handler();
    this.id = config.id ?? `ts-${Date.now().toString(36)}`;
    this.#requestTimeoutMs = positiveTimeout(config.requestTimeoutMs ?? 30_000, "request timeout");
    this.#closeTimeoutMs = positiveTimeout(config.closeTimeoutMs ?? 5_000, "close timeout");
    validateKeepalive(config.keepalive);
    this.#dispatch = new AsyncQueue(config.dispatchCapacity ?? 256);
    this.audio = new AudioStream(
      config.audioBufferBytes,
      config.audioObserver,
      (error) => this.requestStop(error.code === "closed" ? undefined : error, error.code !== "closed"),
    );
    this.#readyPromise = new Promise<void>((resolve, reject) => {
      this.#readyResolve = resolve;
      this.#readyReject = reject;
    });
    void this.#readyPromise.catch(() => {});
    this.#stoppedPromise = new Promise<void>((resolve) => { this.#stoppedResolve = resolve; });
  }

  get state(): SessionState {
    return this.#state;
  }

  /** Connected transport, available after `ready` for binding-specific diagnostics. */
  get transport(): Transport | undefined {
    return this.#transport;
  }

  get ready(): Promise<void> {
    return this.#readyPromise;
  }

  get pendingRequestCount(): number {
    return this.#pending.size;
  }

  run(signal?: AbortSignal): Promise<void> {
    if (this.#runPromise !== undefined) {
      return Promise.reject(new SessionError("already_run", "session has already run"));
    }
    this.#runPromise = this.#supervise(signal);
    return this.#runPromise;
  }

  async close(): Promise<void> {
    if (this.#runPromise === undefined) throw new SessionError("closed", "session has not run");
    this.requestStop();
    await this.#runPromise;
  }

  async request(
    method: string,
    params: WireEncodable,
    options: RequestOptions = {},
  ): Promise<unknown> {
    this.#requireConnected();
    throwIfAborted(options.signal);
    if (method.length === 0) throw new SessionError("configuration", "request method is required");
    const id = this.#nextId();
    const timeoutMs = positiveTimeout(options.timeoutMs ?? this.#requestTimeoutMs, "request timeout");
    let pending!: PendingRequest;
    const result = new Promise<unknown>((resolve, reject) => {
      pending = {
        resolve,
        reject,
        terminal: options.terminal === true,
        ...(options.signal === undefined ? {} : { signal: options.signal }),
      };
    });
    pending.timer = setTimeout(() => {
      this.#cancelPending(
        id,
        pending,
        new SessionError("request_timeout", `request '${method}' timed out after ${timeoutMs}ms`),
      );
    }, timeoutMs);
    if (options.signal !== undefined) {
      pending.onAbort = () => {
        this.#cancelPending(id, pending, new SessionError("aborted", "request aborted", options.signal?.reason));
      };
      options.signal.addEventListener("abort", pending.onAbort, { once: true });
    }
    if (this.#pending.has(id)) throw new SessionError("request_duplicate", `duplicate request id '${id}'`);
    this.#pending.set(id, pending);
    try {
      await this.#sendFrame({ kind: "request", id, method, params });
    } catch (error) {
      this.#cancelPending(id, pending, error);
    }
    return await result;
  }

  async notify(event: string, data: WireEncodable, options: NotifyOptions = {}): Promise<void> {
    this.#requireConnected();
    throwIfAborted(options.signal);
    if (event.length === 0) throw new SessionError("configuration", "event name is required");
    await this.#sendFrame({ kind: "event", id: this.#nextId(), event, data });
  }

  async openAudio(format: MediaFormat): Promise<void> {
    this.#requireConnected();
    const channel = await this.#requireTransport().openMedia("audio", format, this.#abort.signal);
    this.audio.bind(channel);
  }

  async acceptAudio(): Promise<void> {
    this.#requireConnected();
    const channel = await this.#requireTransport().acceptMedia(this.#abort.signal);
    if (channel.id !== "audio") {
      await channel.close();
      throw new SessionError("media_channel", `unexpected media channel '${channel.id}'`);
    }
    this.audio.bind(channel);
  }

  requestStop(cause?: unknown, failed = false): void {
    if (this.#abort.signal.aborted) {
      if (failed) {
        this.#stopCause = this.#stopCause ?? cause;
        this.#stopFailed = true;
      }
      return;
    }
    this.#stopCause = cause;
    this.#stopFailed = failed;
    if (this.#state !== "inactive") this.#state = "closing";
    this.#abort.abort(cause);
    this.#stoppedResolve();
  }

  async sendResponse(
    correlationId: string,
    result?: WireEncodable,
    error?: WireErrorValue,
  ): Promise<void> {
    await this.#sendFrame({
      kind: "response",
      correlationId,
      ...(result === undefined ? {} : { result }),
      ...(error === undefined ? {} : { error }),
    });
  }

  async #supervise(parentSignal?: AbortSignal): Promise<void> {
    this.#state = "connecting";
    const onParentAbort = (): void => this.requestStop(parentSignal?.reason);
    parentSignal?.addEventListener("abort", onParentAbort, { once: true });
    let reader: Promise<void> | undefined;
    let dispatcher: Promise<void> | undefined;
    let keepalive: Promise<void> | undefined;
    let monitor: Promise<void> | undefined;
    try {
      throwIfAborted(parentSignal);
      this.#transport = this.#config.transport
        ?? await this.#config.transportFactory!(this.#config.envelope, this.#abort.signal);
      reader = this.#runWorker(async () => await this.#readControl(), "control_read");
      dispatcher = this.#runWorker(async () => await this.#dispatchControl(), "dispatch");
      const transport = this.#requireTransport();
      if (transport.supportsKeepalive === true && this.#config.keepalive !== undefined) {
        if (transport.monitorKeepalive === undefined) {
          throw new SessionError("configuration", "transport declares keepalive without a monitor");
        }
        keepalive = this.#runWorker(
          async () => await transport.monitorKeepalive!(this.#config.keepalive!, this.#abort.signal),
          "keepalive",
        );
      }
      if (transport.monitor !== undefined) {
        monitor = this.#runWorker(async () => await transport.monitor!(this.#abort.signal), "transport_health");
      }
      const context = new SessionContext(this, this.#abort.signal);
      const beginning = this.#handler.begin(context);
      void beginning.catch(() => {});
      const began = await Promise.race([
        beginning.then(() => true),
        this.#stoppedPromise.then(() => false),
      ]);
      if (!began) throw new SessionError("closed", "session stopped while starting");
      if (this.#abort.signal.aborted) throw new SessionError("closed", "session stopped while starting");
      this.#state = "active";
      this.#readyResolve();
      await this.#stoppedPromise;
    } catch (error) {
      if (!this.#abort.signal.aborted) this.requestStop(error, true);
      if (this.#state === "connecting") this.#readyReject(error);
    } finally {
      parentSignal?.removeEventListener("abort", onParentAbort);
      if (!this.#abort.signal.aborted) this.requestStop();
      this.#dispatch.close();
      this.#failPending(
        this.#stopCause ?? new SessionError("closed", "session is closed"),
      );
      await this.audio.close().catch((error: unknown) => {
        this.#stopCause = this.#stopCause ?? error;
        this.#stopFailed = true;
      });
      const transport = this.#transport;
      if (transport !== undefined) {
        await withTimeout(
          transport.close(),
          this.#closeTimeoutMs,
          "close_timeout",
          "transport close timed out",
        ).catch((error: unknown) => {
          this.#stopCause = this.#stopCause ?? error;
          this.#stopFailed = true;
        });
      }
      const workers = [reader, dispatcher, keepalive, monitor]
        .filter((task): task is Promise<void> => task !== undefined);
      await withTimeout(
        Promise.allSettled(workers).then(() => undefined),
        this.#closeTimeoutMs,
        "close_timeout",
        "session workers did not stop",
      ).catch((error: unknown) => {
        this.#stopCause = this.#stopCause ?? error;
        this.#stopFailed = true;
      });
      this.#state = this.#stopFailed ? "failed" : "closed";
    }
    if (this.#stopFailed) throw asSessionError(this.#stopCause, "session_failed");
  }

  async #runWorker(worker: () => Promise<void>, code: string): Promise<void> {
    try {
      await worker();
      if (!this.#abort.signal.aborted) {
        this.requestStop(new SessionError(code, `${code} worker stopped unexpectedly`), true);
      }
    } catch (error) {
      if (this.#abort.signal.aborted) return;
      const sessionError = asSessionError(error, code);
      if (sessionError.code === "closed") this.requestStop();
      else this.requestStop(sessionError, true);
    }
  }

  async #readControl(): Promise<void> {
    const control = this.#requireTransport().control;
    while (!this.#abort.signal.aborted) {
      const received = await control.receive(this.#abort.signal);
      let frame: ControlFrame;
      try {
        frame = this.#config.envelope.decode(received.data);
      } catch {
        continue;
      }
      if (frame.kind === "response") {
        this.#completePending(frame);
      } else {
        await this.#dispatch.push(frame, this.#abort.signal);
      }
    }
  }

  async #dispatchControl(): Promise<void> {
    while (!this.#abort.signal.aborted) {
      const frame = await this.#dispatch.shift(this.#abort.signal);
      if (frame.kind === "request") await this.#handleRequest(frame);
      else await this.#handleEvent(frame);
    }
  }

  async #handleRequest(frame: Extract<ControlFrame, { kind: "request" }>): Promise<void> {
    const reply: ReplyState = { requestId: frame.id, status: "unclaimed" };
    const context = new SessionContext(this, this.#abort.signal, reply, Date.now());
    try {
      const dispatched = await this.#handler.dispatchRequest(context, {
        method: frame.method,
        payload: frame.params ?? {},
      });
      if (context.replyStatus === "unclaimed" && dispatched !== undefined) {
        if (dispatched.terminal) await context.respondThenClose(dispatched.result);
        else await context.respond(dispatched.result);
      } else if (context.replyStatus === "unclaimed") {
        throw new Error("request handler returned without responding or deferring");
      }
    } catch (error) {
      if (context.replyStatus !== "sent") {
        try {
          await context.respondError(responseError(error));
        } catch (responseFailure) {
          if (!this.#abort.signal.aborted) this.requestStop(responseFailure, true);
        }
      }
    }
  }

  async #handleEvent(frame: Extract<ControlFrame, { kind: "event" }>): Promise<void> {
    const context = new SessionContext(this, this.#abort.signal, undefined, Date.now());
    try {
      await this.#handler.dispatchEvent(context, { event: frame.event, data: frame.data ?? {} });
    } catch {
      // Events are one-way; handler failures do not fabricate acknowledgements.
    }
  }

  async #sendFrame(frame: ControlFrame): Promise<void> {
    if (this.#abort.signal.aborted || this.#state === "closing" || this.#state === "closed" || this.#state === "failed") {
      throw new SessionError("closed", "session is closed");
    }
    const encoded = this.#config.envelope.encode(frame);
    await this.#requireTransport().control.send(encoded, this.#abort.signal);
  }

  #completePending(frame: Extract<ControlFrame, { kind: "response" }>): void {
    const pending = this.#pending.get(frame.correlationId);
    if (pending === undefined) return;
    this.#pending.delete(frame.correlationId);
    detachPending(pending);
    if (frame.error !== undefined) pending.reject(new RemoteError(frame.error));
    else pending.resolve(frame.result);
    if (pending.terminal) this.requestStop();
  }

  #cancelPending(id: string, expected: PendingRequest, error: unknown): void {
    if (this.#pending.get(id) !== expected) return;
    this.#pending.delete(id);
    detachPending(expected);
    expected.reject(error);
  }

  #failPending(error: unknown): void {
    for (const [id, pending] of this.#pending) {
      this.#pending.delete(id);
      detachPending(pending);
      pending.reject(error);
    }
  }

  #nextId(): string {
    this.#sequence += 1;
    return `${this.id}-${this.#sequence.toString(36)}`;
  }

  #requireConnected(): void {
    if (this.#state !== "connecting" && this.#state !== "active") {
      throw new SessionError("inactive", `session is ${this.#state}`);
    }
  }

  #requireTransport(): Transport {
    if (this.#transport === undefined) throw new SessionError("inactive", "transport is not connected");
    return this.#transport;
  }
}
