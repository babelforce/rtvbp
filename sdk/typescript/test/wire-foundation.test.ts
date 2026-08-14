import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { decodeWireJson, encodeWireJson } from "../src/wire.ts";
import type { WireEncodable } from "../src/wire.ts";

const here = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(here, "../../..");
const goldenRoot = join(repositoryRoot, "conformance/babelforce.v1/golden");

function jsonFiles(root: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) files.push(...jsonFiles(path));
    if (entry.isFile() && entry.name.endsWith(".json")) files.push(path);
  }
  return files.sort();
}

test("the JavaScript wire foundation byte-round-trips every frozen fixture", () => {
  const fixtures = jsonFiles(goldenRoot);
  assert.equal(fixtures.length, 48, "the frozen fixture inventory changed");

  for (const fixture of fixtures) {
    const bytes = readFileSync(fixture, "utf8");
    assert.equal(encodeWireJson(decodeWireJson(bytes)), bytes, fixture);
  }
});

test("unsafe integers fail before JavaScript can round them", () => {
  const bytes = '{"value":9007199254740993}';
  assert.equal((JSON.parse(bytes) as { value: number }).value, 9007199254740992);
  assert.throws(() => decodeWireJson(bytes), /safe JavaScript number/);
});

test("non-finite, underflowed, negative-zero, and duplicate-key inputs fail closed", () => {
  for (const bytes of [
    '{"value":1e500}',
    '{"value":1e-500}',
    '{"value":-0}',
    '{"value":1,"value":2}',
  ]) {
    assert.throws(() => decodeWireJson(bytes), bytes);
  }
});

test("encoding rejects values JSON.stringify would silently change", () => {
  for (const value of [
    { value: Number.NaN },
    { value: Number.POSITIVE_INFINITY },
    { value: Number.MAX_SAFE_INTEGER + 1 },
    { value: -0 },
    { value: undefined },
    new Date("2026-08-14T00:00:00Z"),
  ]) {
    assert.throws(() => encodeWireJson(value as unknown as WireEncodable), String(value));
  }
});
