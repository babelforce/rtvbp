import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test, { after, before } from "node:test";

import {
  Handler,
  Session,
  babelforceV1,
  classicV1,
  type MediaFormat,
} from "../src/index.ts";
import { NodeWebSocketServer, nodeWebSocketTransport } from "../src/node.ts";
import { type PeerCommand, startPeer, stopAllPeers } from "./support/peer_process.ts";

const goPeerDirectory = fileURLToPath(new URL("./interop/go", import.meta.url));

// The Go peer is compiled once, before any test, and the tests start the binary. `go run` inside a
// test spent the test's budget compiling, and left its compiled child running when the test timed
// out.
let goPeerBinary = "";
let buildDirectory = "";

before(async () => {
  buildDirectory = await mkdtemp(join(tmpdir(), "rtvbp-interop-"));
  goPeerBinary = join(buildDirectory, "rtvbp-interop-go");
  const build = spawn("go", ["build", "-o", goPeerBinary, "."], {
    cwd: goPeerDirectory,
    stdio: ["ignore", "ignore", "pipe"],
  });
  const stderr: string[] = [];
  build.stderr.on("data", (data: Buffer) => stderr.push(data.toString("utf8")));
  await new Promise<void>((resolve, reject) => {
    build.once("error", reject);
    build.once("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`go build of the interop peer exited with ${code}: ${stderr.join("")}`));
    });
  });
}, { timeout: 110_000 });

after(async () => {
  stopAllPeers();
  if (buildDirectory !== "") await rm(buildDirectory, { recursive: true, force: true });
});

const peers: readonly { readonly name: string; command(): PeerCommand }[] = [
  {
    name: "Go",
    command: () => ({
      name: "Go",
      command: goPeerBinary,
      arguments: [],
      cwd: goPeerDirectory,
    }),
  },
  {
    name: "Rust",
    command: () => ({
      name: "Rust",
      command: "cargo",
      arguments: ["run", "--quiet", "--locked", "--manifest-path", "Cargo.toml", "--"],
      cwd: fileURLToPath(new URL("./interop/rust", import.meta.url)),
      environment: {
        ...process.env,
        CARGO_TARGET_DIR: fileURLToPath(new URL("../../rust/target", import.meta.url)),
      },
    }),
  },
];

const audioFormat: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

function pcmFrame(sample: number): Uint8Array {
  const frame = new Uint8Array(320);
  const view = new DataView(frame.buffer);
  for (let offset = 0; offset < frame.byteLength; offset += 2) view.setInt16(offset, sample, true);
  return frame;
}

function assertSample(frame: Uint8Array, expected: number): void {
  assert.equal(frame.byteLength, 320);
  assert.ok(frame.some((value) => value !== 0), "audio must be non-silent");
  assert.equal(new DataView(frame.buffer, frame.byteOffset, frame.byteLength).getInt16(0, true), expected);
}

function voiceHandler(): Handler {
  const empty = new Proxy({}, { get: () => async () => {} });
  return new Handler({
    adapter: babelforceV1.voiceAdapter(
      empty as babelforceV1.VoiceHandler,
      empty as babelforceV1.VoiceEventHandler,
    ),
    onBegin: async (context) => await context.acceptAudio(),
  });
}

function applicationHandler(): Handler {
  const requests = {
    async ping(_context: unknown, request: babelforceV1.PingRequest): Promise<babelforceV1.PingResponse> {
      const now = Date.now();
      return {
        t0: request.t0,
        t1: now,
        t2: now + 1,
        owd: Math.max(1, now - request.t0),
        ...(request.data === undefined ? {} : { data: request.data }),
      };
    },
    async sessionTerminate(): Promise<babelforceV1.EmptyResponse> {
      return {};
    },
  } as unknown as babelforceV1.ApplicationHandler;
  const events = new Proxy({}, { get: () => async () => {} }) as babelforceV1.ApplicationEventHandler;
  return new Handler({
    adapter: babelforceV1.applicationAdapter(requests, events),
    onBegin: async (context) => await context.openAudio(audioFormat),
  });
}

for (const peer of peers) {
  test(`TypeScript client interoperates with ${peer.name} server`, { timeout: 60_000 }, async (context) => {
    const peerProcess = startPeer(context, peer.command(), "server");
    const url = await peerProcess.firstLine();
    assert.match(url, /^ws:\/\/127\.0\.0\.1:/);
    const session = new Session({
      envelope: classicV1.classicV1Envelope,
      transportFactory: nodeWebSocketTransport({ url, protocols: [], audioFormat }),
      handler: voiceHandler(),
    });
    const done = session.run();
    await session.ready;
    const request: babelforceV1.PingRequest = { t0: Date.now() };
    const response = await new babelforceV1.ApplicationPeer(session).ping(request);
    assert.equal(response.t0, request.t0);

    assertSample(await session.audio.read(320), 1_200);
    await session.audio.write(pcmFrame(-2_400));
    await new babelforceV1.ApplicationPeer(session).sessionTerminate({ reason: "interop complete" });
    await done;
    await peerProcess.wait();
  });

  test(`${peer.name} client interoperates with TypeScript server`, { timeout: 60_000 }, async (context) => {
    let sessionResolve: (session: Session) => void = () => {};
    const accepted = new Promise<Session>((resolve) => { sessionResolve = resolve; });
    const server = new NodeWebSocketServer({
      audioFormat,
      createHandler: applicationHandler,
      onSession: sessionResolve,
    });
    await server.listen();
    context.after(async () => await server.close());
    const peerProcess = startPeer(context, peer.command(), "client", server.url);
    const session = await accepted;
    await session.ready;
    assertSample(await session.audio.read(320), 1_200);
    await session.audio.write(pcmFrame(-2_400));
    await peerProcess.wait();
    while (session.state === "active") await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
