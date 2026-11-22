import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function untilGone(pids: readonly number[]): Promise<number[]> {
  const deadline = Date.now() + 3_000;
  let survivors = pids.filter(alive);
  while (survivors.length > 0 && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
    survivors = survivors.filter(alive);
  }
  return survivors;
}

test("no peer process survives a test that timed out", { timeout: 40_000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "rtvbp-peer-cleanup-"));
  const pidFile = join(directory, "pids");
  let pids: number[] = [];
  try {
    const fixture = fileURLToPath(new URL("./fixtures/hung_peer.ts", import.meta.url));
    // Under `node --test` this process carries NODE_TEST_CONTEXT; left in the environment it would
    // make the fixture run report to this runner and exit 0.
    const { NODE_TEST_CONTEXT: _context, ...environment } = process.env;
    const run = spawnSync(
      process.execPath,
      ["--experimental-strip-types", "--test", fixture],
      {
        env: { ...environment, RTVBP_HUNG_PEER_PIDFILE: pidFile },
        encoding: "utf8",
        timeout: 30_000,
        killSignal: "SIGKILL",
      },
    );
    assert.equal(run.error, undefined, "the fixture run did not end by itself");
    assert.notEqual(run.status, 0, "the fixture test must fail by timeout");
    assert.match(run.stdout, /timed out after 1500ms/);

    pids = (await readFile(pidFile, "utf8")).split("\n").filter(Boolean).map(Number);
    assert.equal(pids.length, 2, "the peer records its shell and its child");
    assert.deepEqual(await untilGone(pids), [], "a peer process outlived the timed-out test");
  } finally {
    for (const pid of pids) if (alive(pid)) process.kill(pid, "SIGKILL");
    await rm(directory, { recursive: true, force: true });
  }
});
