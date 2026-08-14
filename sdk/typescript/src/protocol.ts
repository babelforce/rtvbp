import type { WireEncodable, WireJsonValue } from "./wire.ts";

export type MaybePromise<T> = T | Promise<T>;

export interface HandlerContext {
  readonly signal: AbortSignal;
}

export interface RequestOptions {
  readonly signal?: AbortSignal;
  readonly terminal?: boolean;
}

export interface NotifyOptions {
  readonly signal?: AbortSignal;
}

export interface Requester {
  request(
    method: string,
    params: WireEncodable,
    options?: RequestOptions,
  ): Promise<unknown>;
}

export interface Notifier {
  notify(event: string, data: WireEncodable, options?: NotifyOptions): Promise<void>;
}

export interface RequestRegistration {
  readonly method: string;
  readonly terminal: boolean;
  handle(context: HandlerContext, payload: unknown): Promise<WireEncodable>;
}

export interface EventRegistration {
  readonly event: string;
  handle(context: HandlerContext, data: unknown): Promise<void>;
}

export interface InboundRequest {
  readonly method: string;
  readonly payload: unknown;
}

export interface InboundEvent {
  readonly event: string;
  readonly data: unknown;
}

export interface UnknownHooks {
  readonly request?: (context: HandlerContext, request: InboundRequest) => void | Promise<void>;
  readonly event?: (context: HandlerContext, event: InboundEvent) => void | Promise<void>;
}

export interface RoleAdapter {
  readonly requests: readonly RequestRegistration[];
  readonly events: readonly EventRegistration[];
  readonly unknown: UnknownHooks;
}

export interface WireErrorValue {
  readonly code: number;
  readonly message: string;
  readonly data?: WireJsonValue;
}

export class ProtocolHandlerError extends Error {
  override readonly name = "ProtocolHandlerError";
  readonly wire: WireErrorValue;

  constructor(wire: WireErrorValue) {
    super(wire.message);
    this.wire = wire;
  }
}
