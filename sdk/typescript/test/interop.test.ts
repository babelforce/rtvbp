import assert from "node:assert/strict";
import test, { after, before } from "node:test";

import {
  Handler,
  Session,
  babelforceV1,
  classicV1,
  type MediaFormat,
} from "../src/index.ts";
import { NodeWebSocketServer, nodeWebSocketTransport } from "../src/node.ts";
import { type BuiltPeers, buildPeers } from "./support/peer_build.ts";
import { type PeerCommand, startPeer, stopAllPeers } from "./support/peer_process.ts";

// Both peers are compiled once, before any test, and the tests start the binaries. `go run` and
// `cargo run` inside a test spent the test's budget compiling, and left their compiled child
// running when the test timed out.
let built: BuiltPeers | undefined;

before(async () => {
  built = await buildPeers();
}, { timeout: 280_000 });

after(async () => {
  stopAllPeers();
  await built?.dispose();
});

const peers: readonly { readonly name: string; command(): PeerCommand }[] = [
  { name: "Go", command: () => built!.go },
  { name: "Rust", command: () => built!.rust },
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
    // The server answers after it has read our frame. Terminating earlier races the server's media
    // pump: the terminate request closes its audio stream while our frame is still in its queue.
    assertSample(await session.audio.read(320), 3_600);
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
