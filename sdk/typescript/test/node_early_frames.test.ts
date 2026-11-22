import assert from "node:assert/strict";
import type { AddressInfo } from "node:net";
import test from "node:test";

import { WebSocketServer } from "ws";

import type { EnvelopeCodec, MediaFormat } from "../src/index.ts";
import { nodeWebSocketTransport } from "../src/node.ts";

const audioFormat: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

const unusedEnvelope = undefined as unknown as EnvelopeCodec;

test("Node client keeps frames the server sends right after the upgrade", { timeout: 30_000 }, async (context) => {
  const server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
  context.after(() => new Promise<void>((resolve) => {
    for (const client of server.clients) client.terminate();
    server.close(() => resolve());
  }));
  await new Promise<void>((resolve) => server.once("listening", () => resolve()));
  const frame = new Uint8Array(320).fill(7);
  server.on("connection", (socket) => {
    socket.send(frame, { binary: true });
    socket.send("hello");
  });
  const { port } = server.address() as AddressInfo;
  const factory = nodeWebSocketTransport({ url: `ws://127.0.0.1:${port}/`, protocols: [], audioFormat });

  for (let round = 0; round < 40; round += 1) {
    const transport = await factory(unusedEnvelope, AbortSignal.timeout(5_000));
    try {
      const channel = await transport.acceptMedia();
      const received = await channel.readFrame(AbortSignal.timeout(2_000));
      assert.deepEqual(received.data, frame, `binary frame lost in round ${round}`);
      const control = await transport.control.receive(AbortSignal.timeout(2_000));
      assert.equal(control.data, "hello", `text frame lost in round ${round}`);
    } finally {
      await transport.close();
    }
  }
});
