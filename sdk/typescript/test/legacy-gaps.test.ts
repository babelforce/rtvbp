import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

interface Gap {
  readonly category: string;
  readonly observed: string;
  readonly required: string;
}

interface Evidence {
  readonly source: { readonly source_sha256: string; readonly test_sha256: string };
  readonly gaps: readonly Gap[];
}

const here = dirname(fileURLToPath(import.meta.url));
const evidencePath = resolve(
  here,
  "../../../conformance/interop/browser-consumer-evidence/evidence.json",
);

function legacyParseControl(text: string): Record<string, unknown> | null {
  try {
    const message = JSON.parse(text) as Record<string, unknown>;
    return message !== null && typeof message === "object" && message.version === "1"
      ? message
      : null;
  } catch {
    return null;
  }
}

test("the migration evidence is source-pinned and inventories every required gap", () => {
  const evidence = JSON.parse(readFileSync(evidencePath, "utf8")) as Evidence;
  assert.equal(
    evidence.source.source_sha256,
    "a39d7bc00853c1b7c0e61d3b00d193e074014081031fa88dea039f7facaaf325",
  );
  assert.equal(
    evidence.source.test_sha256,
    "11d7518074578c6bc52e4a46d82965f26fe4b78636296a36d6befe95e53b67fd",
  );

  const categories = new Set(evidence.gaps.map((gap) => gap.category));
  for (const category of [
    "field-presence-and-order",
    "discriminator-precedence",
    "permissive-response",
    "error-response",
    "unknown-dispatch",
    "correlation",
    "numeric-precision",
  ]) {
    assert.ok(categories.has(category), `missing ${category} evidence`);
  }
});

test("the pinned parser accepts frames that a generated classic.v1 decoder must reject or classify", () => {
  assert.deepEqual(legacyParseControl('{"version":"1"}'), { version: "1" });
  assert.deepEqual(
    legacyParseControl(
      '{"version":"1","event":"mixed","id":"1","data":{},"method":"also-a-request","params":{}}',
    ),
    {
      version: "1",
      event: "mixed",
      id: "1",
      data: {},
      method: "also-a-request",
      params: {},
    },
  );
  assert.equal(
    (legacyParseControl('{"version":"1","value":9007199254740993}') as { value: number }).value,
    9007199254740992,
  );
});
