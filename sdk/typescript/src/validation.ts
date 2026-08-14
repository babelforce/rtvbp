import type { WireEncodable, WireJsonValue } from "./wire.ts";
import { wireMap, wireObject } from "./wire.ts";

export interface ValidationIssue {
  readonly path: string;
  readonly rule: string;
  readonly message: string;
}

export class ProtocolValidationError extends Error {
  override readonly name = "ProtocolValidationError";
  readonly issues: readonly ValidationIssue[];

  constructor(issues: readonly ValidationIssue[]) {
    super(
      issues.length === 0
        ? "protocol validation failed"
        : issues.map((issue) => `${issue.path}: ${issue.message}`).join("; "),
    );
    this.issues = issues;
  }
}

export type Schema = Readonly<Record<string, unknown>>;
export type SchemaRegistry = Readonly<Record<string, Schema>>;

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function issue(
  issues: ValidationIssue[],
  path: string,
  rule: string,
  message: string,
): void {
  issues.push({ path, rule, message });
}

function validateAny(value: unknown, path: string, issues: ValidationIssue[]): void {
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (
      !Number.isFinite(value) ||
      Object.is(value, -0) ||
      (Number.isInteger(value) && !Number.isSafeInteger(value))
    ) {
      issue(issues, path, "number", "must be a safe JSON number");
    }
    return;
  }
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) {
      if (!Object.hasOwn(value, index)) issue(issues, `${path}[${index}]`, "presence", "must not be sparse");
      else validateAny(value[index], `${path}[${index}]`, issues);
    }
    return;
  }
  if (isRecord(value)) {
    const prototype = Object.getPrototypeOf(value) as unknown;
    if (prototype !== Object.prototype && prototype !== null) {
      issue(issues, path, "object", "must have a plain or null prototype");
      return;
    }
    for (const key of Object.keys(value)) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (descriptor?.get !== undefined || descriptor?.set !== undefined) {
        issue(issues, `${path}.${key}`, "object", "must not be an accessor");
      } else {
        validateAny(value[key], `${path}.${key}`, issues);
      }
    }
    return;
  }
  issue(issues, path, "type", `unsupported JSON value '${typeof value}'`);
}

function referenceName(reference: unknown): string | undefined {
  return typeof reference === "string" && reference.startsWith("#/schemas/")
    ? reference.slice("#/schemas/".length).replaceAll("~1", "/").replaceAll("~0", "~")
    : undefined;
}

function acceptsNull(schema: Schema): boolean {
  const types = schema["type"];
  if (Array.isArray(types) && types.includes("null")) return true;
  for (const keyword of ["anyOf", "oneOf"] as const) {
    const variants = schema[keyword];
    if (
      Array.isArray(variants) &&
      variants.some((variant) => isRecord(variant) && variant["type"] === "null")
    ) {
      return true;
    }
  }
  return schema["type"] === "null";
}

function nonNullSchema(schema: Schema): Schema {
  const types = schema["type"];
  if (Array.isArray(types)) {
    const nonNull = types.filter((value) => value !== "null");
    if (nonNull.length === 1) return { ...schema, type: nonNull[0] };
  }
  for (const keyword of ["anyOf", "oneOf"] as const) {
    const variants = schema[keyword];
    if (Array.isArray(variants)) {
      const nonNull = variants.filter(
        (variant) => !(isRecord(variant) && variant["type"] === "null"),
      );
      if (nonNull.length === 1 && isRecord(nonNull[0])) return nonNull[0];
    }
  }
  return schema;
}

function validateAt(
  value: unknown,
  schema: Schema,
  schemas: SchemaRegistry,
  path: string,
  issues: ValidationIssue[],
): void {
  if (value === null && acceptsNull(schema)) return;
  schema = nonNullSchema(schema);

  const referenced = referenceName(schema["$ref"]);
  if (referenced !== undefined) {
    const target = schemas[referenced];
    if (target === undefined) {
      issue(issues, path, "schema", `references missing schema '${referenced}'`);
      return;
    }
    validateAt(value, target, schemas, path, issues);
    return;
  }

  switch (schema["type"]) {
    case "null":
      if (value !== null) issue(issues, path, "type", "must be null");
      return;
    case "string": {
      if (typeof value !== "string") {
        issue(issues, path, "type", "must be a string");
        return;
      }
      const minimum = schema["minLength"];
      if (typeof minimum === "number" && [...value].length < minimum) {
        issue(issues, path, "min_length", `must contain at least ${minimum} characters`);
      }
      return;
    }
    case "boolean":
      if (typeof value !== "boolean") issue(issues, path, "type", "must be a boolean");
      return;
    case "integer":
      if (typeof value !== "number" || !Number.isSafeInteger(value) || Object.is(value, -0)) {
        issue(issues, path, "type", "must be a safe JavaScript integer");
        return;
      }
      break;
    case "number":
      if (typeof value !== "number" || !Number.isFinite(value) || Object.is(value, -0)) {
        issue(issues, path, "type", "must be a finite JavaScript number");
        return;
      }
      break;
    case "array": {
      if (!Array.isArray(value)) {
        issue(issues, path, "type", "must be an array");
        return;
      }
      const items = schema["items"];
      if (isRecord(items)) {
        value.forEach((item, index) => validateAt(item, items, schemas, `${path}[${index}]`, issues));
      }
      return;
    }
    case "object": {
      if (!isRecord(value)) {
        issue(issues, path, "type", "must be an object");
        return;
      }
      const properties = schema["properties"];
      const required = new Set(
        Array.isArray(schema["required"])
          ? schema["required"].filter((name): name is string => typeof name === "string")
          : [],
      );
      if (isRecord(properties)) {
        for (const [name, child] of Object.entries(properties)) {
          if (!Object.hasOwn(value, name)) {
            if (required.has(name)) issue(issues, `${path}.${name}`, "required", "is required");
            continue;
          }
          if (isRecord(child)) validateAt(value[name], child, schemas, `${path}.${name}`, issues);
        }
      }
      break;
    }
    default:
      if (schema["type"] === undefined) validateAny(value, path, issues);
      else issue(issues, path, "schema", "uses an unsupported schema shape");
      return;
  }

  if (typeof value === "number") {
    const minimum = schema["minimum"];
    if (typeof minimum === "number" && value < minimum) {
      issue(issues, path, "minimum", `must be at least ${minimum}`);
    }
    const maximum = schema["maximum"];
    if (typeof maximum === "number" && value > maximum) {
      issue(issues, path, "maximum", `must be at most ${maximum}`);
    }
    if (schema["x-rtvbp-nonzero"] === true && value === 0) {
      issue(issues, path, "nonzero", "must be non-zero");
    }
  }

  if (isRecord(value) && Array.isArray(schema["x-rtvbp-field-order"])) {
    for (const ordering of schema["x-rtvbp-field-order"]) {
      if (!isRecord(ordering)) continue;
      const lowerName = ordering["lower"];
      const upperName = ordering["upper"];
      if (typeof lowerName !== "string" || typeof upperName !== "string") continue;
      const lower = value[lowerName];
      const upper = value[upperName];
      if (typeof lower === "number" && typeof upper === "number" && lower > upper) {
        issue(
          issues,
          path,
          "field_order",
          `'${lowerName}' must be less than or equal to '${upperName}'`,
        );
      }
    }
  }
}

export function validateCatalogValue(
  value: unknown,
  schemaName: string,
  schemas: SchemaRegistry,
): readonly ValidationIssue[] {
  const schema = schemas[schemaName];
  if (schema === undefined) {
    return [{ path: "$", rule: "schema", message: `missing schema '${schemaName}'` }];
  }
  const issues: ValidationIssue[] = [];
  validateAt(value, schema, schemas, "$", issues);
  return issues;
}

export function assertCatalogValue(
  value: unknown,
  schemaName: string,
  schemas: SchemaRegistry,
): asserts value is WireJsonValue {
  const issues = validateCatalogValue(value, schemaName, schemas);
  if (issues.length !== 0) throw new ProtocolValidationError(issues);
}

function serializeAt(value: WireJsonValue, schema: Schema, schemas: SchemaRegistry): WireEncodable {
  if (value === null) return null;
  schema = nonNullSchema(schema);
  const referenced = referenceName(schema["$ref"]);
  if (referenced !== undefined) {
    const target = schemas[referenced];
    if (target === undefined) throw new ProtocolValidationError([{ path: "$", rule: "schema", message: `missing schema '${referenced}'` }]);
    return serializeAt(value, target, schemas);
  }
  if (schema["type"] === "array" && Array.isArray(value)) {
    const items = schema["items"];
    return isRecord(items) ? value.map((item) => serializeAt(item, items, schemas)) : value;
  }
  if (schema["type"] === "object" && isRecord(value)) {
    const properties = schema["properties"];
    if (isRecord(properties)) {
      const entries: [string, WireEncodable][] = [];
      for (const [name, child] of Object.entries(properties)) {
        if (Object.hasOwn(value, name) && isRecord(child)) {
          entries.push([name, serializeAt(value[name] as WireJsonValue, child, schemas)]);
        }
      }
      return wireObject(entries);
    }
    return schema["additionalProperties"] === true
      ? wireMap(value as Readonly<Record<string, WireJsonValue>>)
      : wireObject([]);
  }
  return value;
}

export function serializeCatalogValue(
  value: unknown,
  schemaName: string,
  schemas: SchemaRegistry,
): WireEncodable {
  assertCatalogValue(value, schemaName, schemas);
  return serializeAt(value, schemas[schemaName]!, schemas);
}
