import assert from "node:assert/strict";
import test from "node:test";

import {
  Handler,
  Session,
  classicV1,
  demoV1,
  type MediaFormat,
} from "../src/index.ts";
import {
  NodeWebSocketServer,
  nodeWebSocketTransport,
} from "../src/node.ts";
import { WebSocketTransport } from "../src/websocket.ts";

const audioFormat: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

test("Node WebSocket server and client negotiate typed control, Ping/Pong, and duplex L16", async () => {
  let serverSessionResolve: (session: Session) => void = () => {};
  const serverSessionReady = new Promise<Session>((resolve) => { serverSessionResolve = resolve; });
  const server = new NodeWebSocketServer({
    audioFormat,
    authenticate: (request) => request.headers.authorization === "Bearer public-test-token",
    createHandler: () => new Handler({
      adapter: demoV1.applicationAdapter(
        { demoEcho: async (_context, request) => ({ message: request.message }) },
        {},
      ),
    }),
    onSession: serverSessionResolve,
  });
  await server.listen();

  let clientTransport: WebSocketTransport | undefined;
  const factory = nodeWebSocketTransport({
    url: server.url,
    headers: { authorization: "Bearer public-test-token" },
    audioFormat,
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transportFactory: async (envelope, signal) => {
      const transport = await factory(envelope, signal);
      assert.ok(transport instanceof WebSocketTransport);
      clientTransport = transport;
      return transport;
    },
    keepalive: { intervalMs: 5, timeoutMs: 20, maxMisses: 2 },
    handler: new Handler({ adapter: demoV1.voiceAdapter({}, { demoObserved: async () => {} }) }),
  });
  const clientDone = client.run();
  const serverSession = await serverSessionReady;
  await Promise.all([client.ready, serverSession.ready]);

  assert.equal(clientTransport?.wireSubprotocol, "rtvbp.v1");
  assert.equal(clientTransport?.profile, "rtvbp.v1");
  assert.deepEqual(
    await new demoV1.ApplicationPeer(client).demoEcho({ message: "over websocket" }),
    { message: "over websocket" },
  );

  await Promise.all([client.openAudio(audioFormat), serverSession.acceptAudio()]);
  const samples = Uint8Array.from({ length: 320 }, (_, index) => (index % 247) + 1);
  await client.audio.write(samples);
  assert.deepEqual(await serverSession.audio.read(320), samples);
  await serverSession.audio.write(samples);
  assert.deepEqual(await client.audio.read(320), samples);

  await new Promise((resolve) => setTimeout(resolve, 15));
  assert.equal(client.state, "active");
  await client.close();
  await clientDone;
  await server.close();
});

test("headerless v1 fallback works with the opposite generated local role", async () => {
  let observedResolve: (message: string) => void = () => {};
  const observed = new Promise<string>((resolve) => { observedResolve = resolve; });
  const server = new NodeWebSocketServer({
    createHandler: () => new Handler({
      adapter: demoV1.voiceAdapter({}, {
        demoObserved: async (_context, event) => observedResolve(event.message),
      }),
    }),
  });
  await server.listen();

  let transport: WebSocketTransport | undefined;
  const factory = nodeWebSocketTransport({ url: server.url, protocols: [] });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transportFactory: async (envelope, signal) => {
      const connected = await factory(envelope, signal);
      assert.ok(connected instanceof WebSocketTransport);
      transport = connected;
      return connected;
    },
    handler: new Handler({
      adapter: demoV1.applicationAdapter(
        { demoEcho: async (_context, request) => ({ message: request.message }) },
        {},
      ),
    }),
  });
  const done = client.run();
  await client.ready;
  assert.equal(transport?.wireSubprotocol, "");
  assert.equal(transport?.profile, "rtvbp.v1");

  await new demoV1.ApplicationEvents(client).demoObserved({ message: "headerless" });
  assert.equal(await observed, "headerless");
  await client.close();
  await done;
  await server.close();
});
