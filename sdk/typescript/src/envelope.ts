import type { WireEncodable, WireJsonValue } from "./wire.ts";
import { decodeWireJson, encodeWireJson, wireObject } from "./wire.ts";
import type { WireErrorValue } from "./protocol.ts";

export type FrameKind = "request" | "response" | "event";

export type ControlFrame =
  | {
      readonly kind: "request";
      readonly id: string;
      readonly method: string;
      readonly params?: WireEncodable;
    }
  | {
      readonly kind: "response";
      readonly correlationId: string;
      readonly result?: WireEncodable;
      readonly error?: WireErrorValue;
    }
  | {
      readonly kind: "event";
      readonly id: string;
      readonly event: string;
      readonly data?: WireEncodable;
    };

export class EnvelopeError extends Error {
  override readonly name = "EnvelopeError";
}

export interface EnvelopeFieldSpec {
  readonly name: string;
  readonly omitWhenNone: boolean;
}

export interface EnvelopeFrameSpec {
  readonly kind: FrameKind;
  readonly discriminator: EnvelopeFieldSpec;
  readonly id?: EnvelopeFieldSpec;
  readonly payload: EnvelopeFieldSpec;
  readonly error?: EnvelopeFieldSpec;
}

export interface EnvelopeDescriptor {
  readonly id: string;
  readonly constants: readonly (readonly [string, string])[];
  /** Declaration order is structural discrimination precedence. */
  readonly frames: readonly EnvelopeFrameSpec[];
  readonly error: {
    readonly code: EnvelopeFieldSpec;
    readonly message: EnvelopeFieldSpec;
    readonly data: EnvelopeFieldSpec;
  };
}

export interface EnvelopeCodec {
  readonly name: string;
  encode(frame: ControlFrame): string;
  decode(bytes: string): ControlFrame;
}

function objectValue(value: WireJsonValue, message: string): Readonly<Record<string, WireJsonValue>> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new EnvelopeError(message);
  }
  return value as Readonly<Record<string, WireJsonValue>>;
}

function nonEmptyString(
  object: Readonly<Record<string, WireJsonValue>>,
  name: string,
  message: string,
): string {
  const value = object[name];
  if (typeof value !== "string" || value.length === 0) throw new EnvelopeError(message);
  return value;
}

function optionalValue(
  object: Readonly<Record<string, WireJsonValue>>,
  name: string,
): WireJsonValue | undefined {
  return Object.hasOwn(object, name) ? object[name] : undefined;
}

function pushOptional(
  entries: [string, WireEncodable][],
  field: EnvelopeFieldSpec,
  value: WireEncodable | undefined,
): void {
  if (value !== undefined) entries.push([field.name, value]);
  else if (!field.omitWhenNone) entries.push([field.name, null]);
}

export function createEnvelopeCodec(descriptor: EnvelopeDescriptor): EnvelopeCodec {
  const frameSpec = (kind: FrameKind): EnvelopeFrameSpec => {
    const shape = descriptor.frames.find((candidate) => candidate.kind === kind);
    if (shape === undefined) throw new EnvelopeError(`${descriptor.id}: frame mapping is missing`);
    return shape;
  };

  return {
    name: descriptor.id,
    encode(frame): string {
      const shape = frameSpec(frame.kind);
      const entries: [string, WireEncodable][] = descriptor.constants.map(([name, value]) => [
        name,
        value,
      ]);
      if (frame.kind === "request" || frame.kind === "event") {
        if (frame.id.length === 0) throw new EnvelopeError(`${descriptor.id}: frame id is required`);
        const method = frame.kind === "request" ? frame.method : frame.event;
        if (method.length === 0) throw new EnvelopeError(`${descriptor.id}: frame method is required`);
        if (shape.id === undefined) throw new EnvelopeError(`${descriptor.id}: frame id mapping is missing`);
        entries.push([shape.id.name, frame.id], [shape.discriminator.name, method]);
        pushOptional(entries, shape.payload, frame.kind === "request" ? frame.params : frame.data);
      } else {
        if (frame.correlationId.length === 0) {
          throw new EnvelopeError(`${descriptor.id}: response correlation id is required`);
        }
        entries.push([shape.discriminator.name, frame.correlationId]);
        pushOptional(entries, shape.payload, frame.result);
        if (shape.error === undefined) {
          throw new EnvelopeError(`${descriptor.id}: response error mapping is missing`);
        }
        if (frame.error === undefined) {
          pushOptional(entries, shape.error, undefined);
        } else {
          if (!Number.isSafeInteger(frame.error.code) || frame.error.code === 0) {
            throw new EnvelopeError(`${descriptor.id}: error code must be a non-zero safe integer`);
          }
          if (frame.error.message.length === 0) {
            throw new EnvelopeError(`${descriptor.id}: error message is required`);
          }
          const errorEntries: [string, WireEncodable][] = [
            [descriptor.error.code.name, frame.error.code],
            [descriptor.error.message.name, frame.error.message],
          ];
          pushOptional(errorEntries, descriptor.error.data, frame.error.data);
          entries.push([shape.error.name, wireObject(errorEntries)]);
        }
      }
      return encodeWireJson(wireObject(entries));
    },
    decode(bytes): ControlFrame {
      const object = objectValue(decodeWireJson(bytes), `${descriptor.id}: envelope must be an object`);
      for (const [name, expected] of descriptor.constants) {
        if (object[name] !== expected) {
          throw new EnvelopeError(`${descriptor.id}: ${name} must equal ${JSON.stringify(expected)}`);
        }
      }
      for (const shape of descriptor.frames) {
        const discriminator = object[shape.discriminator.name];
        if (typeof discriminator !== "string" || discriminator.length === 0) continue;
        if (shape.kind === "response") {
          let error: WireErrorValue | undefined;
          if (shape.error !== undefined) {
            const rawError = optionalValue(object, shape.error.name);
            if (rawError !== undefined && rawError !== null) {
              const errorObject = objectValue(rawError, `${descriptor.id}: error must be an object`);
              const code = errorObject[descriptor.error.code.name];
              const message = errorObject[descriptor.error.message.name];
              if (typeof code !== "number" || !Number.isSafeInteger(code) || code === 0) {
                throw new EnvelopeError(`${descriptor.id}: error code must be a non-zero safe integer`);
              }
              if (typeof message !== "string" || message.length === 0) {
                throw new EnvelopeError(`${descriptor.id}: error message is required`);
              }
              const data = optionalValue(errorObject, descriptor.error.data.name);
              error = data === undefined ? { code, message } : { code, message, data };
            }
          }
          const result = optionalValue(object, shape.payload.name);
          return {
            kind: "response",
            correlationId: discriminator,
            ...(result === undefined ? {} : { result }),
            ...(error === undefined ? {} : { error }),
          };
        }
        if (shape.id === undefined) throw new EnvelopeError(`${descriptor.id}: frame id mapping is missing`);
        const id = nonEmptyString(object, shape.id.name, `${descriptor.id}: ${shape.kind} id is required`);
        const payload = optionalValue(object, shape.payload.name);
        return shape.kind === "request"
          ? {
              kind: "request",
              id,
              method: discriminator,
              ...(payload === undefined ? {} : { params: payload }),
            }
          : {
              kind: "event",
              id,
              event: discriminator,
              ...(payload === undefined ? {} : { data: payload }),
            };
      }
      throw new EnvelopeError(`${descriptor.id}: envelope has no recognized frame discriminator`);
    },
  };
}
