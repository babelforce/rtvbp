import assert from "node:assert/strict";
import test from "node:test";

import {
  Handler,
  MemoryTransport,
  RemoteError,
  Session,
  SessionError,
  classicV1,
  demoV1,
} from "../src/index.ts";
import type { RoleAdapter } from "../src/protocol.ts";

async function active(session: Session): Promise<void> {
  await session.ready;
  assert.equal(session.state, "active");
}

test("generated peers execute over two memory sessions", async () => {
  const [clientTransport, serverTransport] = MemoryTransport.pair();
  const server = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: serverTransport,
    handler: new Handler({
      adapter: demoV1.applicationAdapter(
        { demoEcho: async (_context, request) => ({ message: request.message }) },
        {},
      ),
    }),
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler({ adapter: demoV1.voiceAdapter({}, { demoObserved: async () => {} }) }),
  });

  const serverDone = server.run();
  const clientDone = client.run();
  await Promise.all([active(server), active(client)]);

  const peer = new demoV1.ApplicationPeer(client);
  assert.deepEqual(await peer.demoEcho({ message: "hello" }), { message: "hello" });

  await client.close();
  await Promise.all([clientDone, serverDone]);
});

test("unknown requests receive 501 and are never implicitly acknowledged", async () => {
  const [clientTransport, serverTransport] = MemoryTransport.pair();
  const server = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: serverTransport,
    handler: new Handler(),
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler(),
  });
  const serverDone = server.run();
  const clientDone = client.run();
  await Promise.all([active(server), active(client)]);

  await assert.rejects(
    client.request("missing.method", {}),
    (error: unknown) => error instanceof RemoteError && error.wire.code === 501,
  );

  await client.close();
  await Promise.all([clientDone, serverDone]);
});

test("request timeout wins once and leaves no pending work", async () => {
  const [clientTransport] = MemoryTransport.pair();
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler(),
    requestTimeoutMs: 10,
  });
  const done = client.run();
  await active(client);

  await assert.rejects(
    client.request("never.responds", {}),
    (error: unknown) => error instanceof SessionError && error.code === "request_timeout",
  );
  assert.equal(client.pendingRequestCount, 0);
  await client.close();
  await done;
});

function adapter(
  requests: RoleAdapter["requests"] = [],
  events: RoleAdapter["events"] = [],
): RoleAdapter {
  return { requests, events, unknown: {} };
}

test("responses bypass serial dispatch so a handler can make a nested request", async () => {
  const [leftTransport, rightTransport] = MemoryTransport.pair();
  const left = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: leftTransport,
    handler: new Handler({
      adapter: adapter([
        {
          method: "outer",
          terminal: false,
          async handle(context) {
            const inner = await context.request("inner", { value: "nested" });
            return { inner: inner as never };
          },
        },
      ]),
    }),
  });
  const right = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: rightTransport,
    handler: new Handler({
      adapter: adapter([
        {
          method: "inner",
          terminal: false,
          async handle(_context, payload) {
            return payload as never;
          },
        },
      ]),
    }),
  });
  const leftDone = left.run();
  const rightDone = right.run();
  await Promise.all([active(left), active(right)]);

  assert.deepEqual(await right.request("outer", {}), { inner: { value: "nested" } });
  await right.close();
  await Promise.all([leftDone, rightDone]);
});

test("request middleware wraps serial dispatch and deferred replies are one-shot", async () => {
  const [clientTransport, serverTransport] = MemoryTransport.pair();
  const sequence: string[] = [];
  let duplicateAttempt: Promise<void> | undefined;
  const server = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: serverTransport,
    handler: new Handler({
      middleware: [async (_context, request, next) => {
        sequence.push(`before:${request.method}`);
        await next();
        sequence.push(`after:${request.method}`);
      }],
      adapter: adapter([
        {
          method: "deferred",
          terminal: false,
          async handle(context) {
            const deferred = context.deferResponse();
            queueMicrotask(() => {
              duplicateAttempt = deferred.respond({ ok: true }).then(async () => {
                await assert.rejects(deferred.respond({ ok: false }), SessionError);
              });
            });
            return {};
          },
        },
      ]),
    }),
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler(),
  });
  const serverDone = server.run();
  const clientDone = client.run();
  await Promise.all([active(server), active(client)]);

  assert.deepEqual(await client.request("deferred", {}), { ok: true });
  assert.ok(duplicateAttempt);
  await duplicateAttempt;
  assert.deepEqual(sequence, ["before:deferred", "after:deferred"]);
  await client.close();
  await Promise.all([clientDone, serverDone]);
});

test("inbound requests dispatch serially while the response reader remains independent", async () => {
  const [clientTransport, serverTransport] = MemoryTransport.pair();
  const started: number[] = [];
  let releaseFirst: () => void = () => {};
  const firstGate = new Promise<void>((resolve) => { releaseFirst = resolve; });
  const server = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: serverTransport,
    handler: new Handler({
      adapter: adapter([
        {
          method: "sequence",
          terminal: false,
          async handle(_context, payload) {
            const order = (payload as { readonly order: number }).order;
            started.push(order);
            if (order === 1) await firstGate;
            return { order };
          },
        },
      ]),
    }),
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler(),
  });
  const serverDone = server.run();
  const clientDone = client.run();
  await Promise.all([active(server), active(client)]);

  const first = client.request("sequence", { order: 1 });
  const second = client.request("sequence", { order: 2 });
  while (started.length === 0) await new Promise((resolve) => setTimeout(resolve, 0));
  assert.deepEqual(started, [1]);
  releaseFirst();
  assert.deepEqual(await Promise.all([first, second]), [{ order: 1 }, { order: 2 }]);
  assert.deepEqual(started, [1, 2]);

  await client.close();
  await Promise.all([clientDone, serverDone]);
});

test("terminal handlers flush their response before both sessions close", async () => {
  const [clientTransport, serverTransport] = MemoryTransport.pair();
  const server = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: serverTransport,
    handler: new Handler({
      adapter: adapter([
        { method: "finish", terminal: true, async handle() { return { flushed: true }; } },
      ]),
    }),
  });
  const client = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: clientTransport,
    handler: new Handler(),
  });
  const serverDone = server.run();
  const clientDone = client.run();
  await Promise.all([active(server), active(client)]);

  assert.deepEqual(await client.request("finish", {}, { terminal: true }), { flushed: true });
  await Promise.all([clientDone, serverDone]);
  assert.equal(client.state, "closed");
  assert.equal(server.state, "closed");
});

test("session audio frames arbitrary writes and provides duplex timing, clear, and cancellation", async () => {
  const [leftTransport, rightTransport] = MemoryTransport.pair();
  const observed: string[] = [];
  const left = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: leftTransport,
    handler: new Handler(),
    audioBufferBytes: 960,
    audioObserver: (frame) => observed.push(`${frame.direction}:${frame.data.byteLength}`),
  });
  const right = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: rightTransport,
    handler: new Handler(),
    audioBufferBytes: 960,
  });
  const leftDone = left.run();
  const rightDone = right.run();
  await Promise.all([active(left), active(right)]);
  const format = {
    encoding: "L16",
    sampleRate: 8_000,
    bitDepth: 16,
    channels: 1,
    packetTimeMs: 20,
  } as const;
  await Promise.all([left.openAudio(format), right.acceptAudio()]);

  const outbound = Uint8Array.from({ length: 320 }, (_, index) => (index % 251) + 1);
  assert.equal(await left.audio.write(outbound.subarray(0, 31)), 31);
  assert.equal(await left.audio.write(outbound.subarray(31)), 289);
  assert.deepEqual(await right.audio.read(320), outbound);

  await right.audio.write(outbound);
  assert.deepEqual(await left.audio.read(320), outbound);
  assert.ok(observed.includes("out:320"));
  assert.ok(observed.includes("in:320"));

  await left.audio.write(outbound);
  while (right.audio.bufferedInboundBytes === 0) await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(right.audio.clear(), 320);

  const abort = new AbortController();
  const blocked = right.audio.read(1, abort.signal);
  abort.abort();
  await assert.rejects(
    blocked,
    (error: unknown) => error instanceof SessionError && error.code === "aborted",
  );

  await left.close();
  await Promise.all([leftDone, rightDone]);
});
