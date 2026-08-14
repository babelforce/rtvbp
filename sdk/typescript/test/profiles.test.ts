import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  DEFAULT_PROFILE,
  HEADERLESS_PROFILE,
  PROFILES,
  PROFILE_RTVBP_DEMO_V1,
  PROFILE_RTVBP_V1,
  PROFILE_RTVBP_WEBRTC_V1,
  SERVER_PREFERENCE,
} from "../src/generated/zz_generated_profiles.ts";

interface NegotiationCase {
  readonly name: string;
  readonly offered: readonly string[];
  readonly selectedToken?: string | null;
  readonly effectiveProfile?: string;
  readonly error?: string;
}

interface NegotiationVectors {
  readonly valid: readonly NegotiationCase[];
  readonly invalid: readonly NegotiationCase[];
}

const here = dirname(fileURLToPath(import.meta.url));
const vectors = JSON.parse(
  readFileSync(resolve(here, "../../../conformance/profiles/negotiation.json"), "utf8"),
) as NegotiationVectors;

function select(offered: readonly string[]): {
  readonly selectedToken: string | null;
  readonly effectiveProfile: string;
} {
  if (offered.length === 0) {
    return { selectedToken: null, effectiveProfile: HEADERLESS_PROFILE };
  }
  if (offered.some((token) => token.length === 0)) throw new Error("invalid-token");
  const selected = SERVER_PREFERENCE.find((profile) => offered.includes(profile));
  if (selected === undefined) throw new Error("unsupported-profile");
  return { selectedToken: selected, effectiveProfile: selected };
}

test("generated TypeScript profiles reproduce current composition and preference", () => {
  assert.deepEqual(SERVER_PREFERENCE, [
    PROFILE_RTVBP_V1,
    PROFILE_RTVBP_DEMO_V1,
    PROFILE_RTVBP_WEBRTC_V1,
  ]);
  assert.equal(DEFAULT_PROFILE, PROFILE_RTVBP_V1);
  assert.equal(HEADERLESS_PROFILE, PROFILE_RTVBP_V1);
  assert.deepEqual(
    PROFILES.map(({ id, transport, envelope, catalog }) => ({ id, transport, envelope, catalog })),
    [
      { id: PROFILE_RTVBP_V1, transport: "ws.v1", envelope: "classic.v1", catalog: "babelforce.v1" },
      { id: PROFILE_RTVBP_DEMO_V1, transport: "ws.v1", envelope: "classic.v1", catalog: "demo.v1" },
      {
        id: PROFILE_RTVBP_WEBRTC_V1,
        transport: "webrtcws.v1",
        envelope: "classic.v1",
        catalog: "babelforce.v1",
      },
    ],
  );
});

test("generated valid and invalid negotiation vectors execute against generated constants", () => {
  for (const vector of vectors.valid) {
    assert.deepEqual(select(vector.offered), {
      selectedToken: vector.selectedToken,
      effectiveProfile: vector.effectiveProfile,
    }, vector.name);
  }
  for (const vector of vectors.invalid) {
    assert.throws(() => select(vector.offered), new RegExp(vector.error ?? "error"), vector.name);
  }
});
