import type { WireErrorValue } from "./protocol.ts";

export class SessionError extends Error {
  override readonly name: string = "SessionError";
  readonly code: string;
  override readonly cause?: unknown;

  constructor(code: string, message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause });
    this.code = code;
    this.cause = cause;
  }
}

export class RemoteError extends SessionError {
  override readonly name = "RemoteError";
  readonly wire: WireErrorValue;

  constructor(wire: WireErrorValue) {
    super("remote", wire.message);
    this.wire = wire;
  }
}

export function aborted(signal?: AbortSignal): SessionError {
  return new SessionError("aborted", "operation aborted", signal?.reason);
}

export function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted === true) throw aborted(signal);
}

export function asSessionError(error: unknown, code = "internal"): SessionError {
  if (error instanceof SessionError) return error;
  return new SessionError(code, error instanceof Error ? error.message : String(error), error);
}
