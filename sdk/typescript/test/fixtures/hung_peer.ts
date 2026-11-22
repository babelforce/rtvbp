// Run by interop_cleanup.test.ts as its own `node --test` process, never by the suite glob. It
// starts a peer that never prints a URL and lets the test time out, as `go run` did while it
// waited for a build slot.
import test from "node:test";

import { startPeer } from "../support/peer_process.ts";

const pidFile = process.env["RTVBP_HUNG_PEER_PIDFILE"];
if (pidFile === undefined) throw new Error("RTVBP_HUNG_PEER_PIDFILE is not set");

test("a peer that never answers", { timeout: 1_500 }, async (context) => {
  const peer = startPeer(
    context,
    {
      name: "hung",
      command: "sh",
      arguments: ["-c", 'sleep 300 & echo $! > "$1"; echo $$ >> "$1"; wait', "peer", pidFile],
      cwd: process.cwd(),
    },
    "server",
  );
  await peer.firstLine();
});
