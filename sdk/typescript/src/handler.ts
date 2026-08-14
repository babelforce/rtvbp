import { SessionError } from "./errors.ts";
import type {
  HandlerContext,
  InboundEvent,
  InboundRequest,
  MaybePromise,
  RoleAdapter,
} from "./protocol.ts";
import { ProtocolHandlerError } from "./protocol.ts";
import { ProtocolValidationError } from "./validation.ts";
import type { WireEncodable } from "./wire.ts";

export type RequestMiddleware = (
  context: HandlerContext,
  request: InboundRequest,
  next: () => Promise<void>,
) => MaybePromise<void>;

export interface HandlerConfig {
  readonly adapter?: RoleAdapter;
  readonly onBegin?: (context: HandlerContext) => MaybePromise<void>;
  readonly middleware?: readonly RequestMiddleware[];
}

export interface RequestDispatch {
  readonly result: WireEncodable;
  readonly terminal: boolean;
}

const EMPTY_ADAPTER: RoleAdapter = { requests: [], events: [], unknown: {} };

/** Runtime dispatch over generated role registrations. */
export class Handler {
  readonly #adapter: RoleAdapter;
  readonly #requests = new Map<string, RoleAdapter["requests"][number]>();
  readonly #events = new Map<string, RoleAdapter["events"][number]>();
  readonly #onBegin: ((context: HandlerContext) => MaybePromise<void>) | undefined;
  readonly #middleware: readonly RequestMiddleware[];

  constructor(config: HandlerConfig = {}) {
    this.#adapter = config.adapter ?? EMPTY_ADAPTER;
    this.#onBegin = config.onBegin;
    this.#middleware = config.middleware ?? [];
    for (const request of this.#adapter.requests) {
      if (this.#requests.has(request.method)) {
        throw new SessionError("configuration", `duplicate request handler '${request.method}'`);
      }
      this.#requests.set(request.method, request);
    }
    for (const event of this.#adapter.events) {
      if (this.#events.has(event.event)) {
        throw new SessionError("configuration", `duplicate event handler '${event.event}'`);
      }
      this.#events.set(event.event, event);
    }
  }

  async begin(context: HandlerContext): Promise<void> {
    await this.#onBegin?.(context);
  }

  async dispatchRequest(
    context: HandlerContext,
    request: InboundRequest,
  ): Promise<RequestDispatch | undefined> {
    let dispatched: RequestDispatch | undefined;
    const registered = this.#requests.get(request.method);
    const invoke = async (): Promise<void> => {
      if (registered !== undefined) {
        try {
          const result = await registered.handle(context, request.payload);
          dispatched = { result, terminal: registered.terminal };
        } catch (error) {
          if (error instanceof ProtocolValidationError) {
            throw new ProtocolHandlerError({ code: 400, message: error.message });
          }
          throw error;
        }
        return;
      }
      if (this.#adapter.unknown.request === undefined) {
        throw new ProtocolHandlerError({
          code: 501,
          message: `unknown method: ${request.method}`,
        });
      }
      await this.#adapter.unknown.request(context, request);
    };

    let next = invoke;
    for (const middleware of [...this.#middleware].reverse()) {
      const downstream = next;
      next = async () => await middleware(context, request, downstream);
    }
    await next();
    return dispatched;
  }

  async dispatchEvent(context: HandlerContext, event: InboundEvent): Promise<void> {
    const registered = this.#events.get(event.event);
    if (registered !== undefined) {
      await registered.handle(context, event.data);
      return;
    }
    await this.#adapter.unknown.event?.(context, event);
  }
}
