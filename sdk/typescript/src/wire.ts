import { isSafeNumber, parse } from "lossless-json";

export type WireJsonPrimitive = null | boolean | number | string;
export type WireJsonValue =
  | WireJsonPrimitive
  | readonly WireJsonValue[]
  | { readonly [key: string]: WireJsonValue };

const orderedWireObject = Symbol("orderedWireObject");

export interface OrderedWireObject {
  readonly [orderedWireObject]: true;
  readonly entries: readonly (readonly [string, WireEncodable])[];
}

export type WireEncodable = WireJsonValue | OrderedWireObject;

export class WireJsonError extends Error {
  override readonly name = "WireJsonError";
}

/** Build a JSON object whose field order is part of the frozen wire contract. */
export function wireObject(
  entries: readonly (readonly [string, WireEncodable])[],
): OrderedWireObject {
  return { [orderedWireObject]: true, entries };
}

/** Build a Go-compatible open map: keys are emitted in Unicode code-point order. */
export function wireMap(values: Readonly<Record<string, WireEncodable>>): OrderedWireObject {
  return wireObject(
    Object.keys(values)
      .sort()
      .map((key) => [key, values[key]!] as const),
  );
}

function parseNumber(value: string): number {
  if (!isSafeNumber(value, { approx: true })) {
    throw new WireJsonError(`JSON number '${value}' is not a safe JavaScript number`);
  }

  const parsed = Number(value);
  if (
    !Number.isFinite(parsed) ||
    Object.is(parsed, -0) ||
    (Number.isInteger(parsed) && !Number.isSafeInteger(parsed))
  ) {
    throw new WireJsonError(`JSON number '${value}' is not a safe JavaScript number`);
  }
  return parsed;
}

/**
 * Decode JSON without allowing the native parser to round an integer before validation.
 * Duplicate keys and numeric values outside the documented JavaScript domain fail closed.
 */
export function decodeWireJson(bytes: string): WireJsonValue {
  try {
    return parse(bytes, undefined, { parseNumber }) as WireJsonValue;
  } catch (error) {
    if (error instanceof WireJsonError) throw error;
    const detail = error instanceof Error ? error.message : String(error);
    throw new WireJsonError(`invalid RTVBP JSON: ${detail}`);
  }
}

function isOrderedWireObject(value: object): value is OrderedWireObject {
  return orderedWireObject in value;
}

function encodeValue(value: WireEncodable, ancestors: Set<object>): string {
  if (value === null) return "null";

  switch (typeof value) {
    case "string":
      return JSON.stringify(value);
    case "boolean":
      return value ? "true" : "false";
    case "number":
      if (
        !Number.isFinite(value) ||
        Object.is(value, -0) ||
        (Number.isInteger(value) && !Number.isSafeInteger(value))
      ) {
        throw new WireJsonError(`number '${String(value)}' is not a safe JavaScript number`);
      }
      return JSON.stringify(value);
    case "bigint":
    case "function":
    case "symbol":
    case "undefined":
      throw new WireJsonError(`unsupported JSON value: ${typeof value}`);
    case "object":
      break;
  }

  if (ancestors.has(value)) throw new WireJsonError("cyclic JSON value");
  ancestors.add(value);
  try {
    if (Array.isArray(value)) {
      const encoded: string[] = [];
      for (let index = 0; index < value.length; index += 1) {
        if (!Object.hasOwn(value, index)) throw new WireJsonError("sparse JSON array");
        encoded.push(encodeValue(value[index]!, ancestors));
      }
      return `[${encoded.join(",")}]`;
    }

    let entries: readonly (readonly [string, WireEncodable])[];
    if (isOrderedWireObject(value)) {
      entries = value.entries;
    } else {
      const prototype = Object.getPrototypeOf(value) as unknown;
      if (prototype !== Object.prototype && prototype !== null) {
        throw new WireJsonError("JSON objects must have a plain or null prototype");
      }
      entries = Object.keys(value).map((key) => {
        const descriptor = Object.getOwnPropertyDescriptor(value, key);
        if (descriptor?.get !== undefined || descriptor?.set !== undefined) {
          throw new WireJsonError(`JSON property '${key}' must not be an accessor`);
        }
        return [key, (value as Readonly<Record<string, WireEncodable>>)[key]!] as const;
      });
    }

    const seenKeys = new Set<string>();
    const encoded = entries.map(([key, entry]) => {
      if (seenKeys.has(key)) throw new WireJsonError(`duplicate JSON key '${key}'`);
      seenKeys.add(key);
      return `${JSON.stringify(key)}:${encodeValue(entry, ancestors)}`;
    });
    return `{${encoded.join(",")}}`;
  } finally {
    ancestors.delete(value);
  }
}

/** Encode only values that JSON can represent without silent coercion or numeric corruption. */
export function encodeWireJson(value: WireEncodable): string {
  return encodeValue(value, new Set());
}
