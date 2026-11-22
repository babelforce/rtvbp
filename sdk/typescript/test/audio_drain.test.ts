import assert from "node:assert/strict";
import test from "node:test";

import {
  Handler,
  MemoryTransport,
  Session,
  SessionError,
  aborted,
  classicV1,
  type MediaChannel,
  type MediaFrame,
  type Transport,
} from "../src/index.ts";

const format = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
} as const;

/** Holds every inbound frame in the transport queue until the test opens the gate. */
function gateMedia(inner: MediaChannel, gate: Promise<void>): MediaChannel {
  return {
    id: inner.id,
    format: inner.format,
    writeFrame: async (frame, signal) => await inner.writeFrame(frame, signal),
    readFrame: async (signal): Promise<MediaFrame> => {
      await new Promise<void>((resolve, reject) => {
        const onAbort = (): void => reject(aborted(signal));
        if (signal?.aborted === true) return onAbort();
        signal?.addEventListener("abort", onAbort, { once: true });
        void gate.then(() => {
          signal?.removeEventListener("abort", onAbort);
          resolve();
        });
      });
      return await inner.readFrame(signal);
    },
    close: async () => await inner.close(),
  };
}

function gateTransport(inner: Transport, gate: Promise<void>): Transport {
  return {
    control: inner.control,
    openMedia: async (id, mediaFormat, signal) => await inner.openMedia(id, mediaFormat, signal),
    acceptMedia: async (signal) => gateMedia(await inner.acceptMedia(signal), gate),
    close: async () => await inner.close(),
  };
}

test("inbound audio queued before shutdown is still delivered, then the stream ends", { timeout: 20_000 }, async () => {
  const [leftTransport, rightTransport] = MemoryTransport.pair();
  let openGate: () => void = () => {};
  const gate = new Promise<void>((resolve) => { openGate = resolve; });
  const left = new Session({ envelope: classicV1.classicV1Envelope, transport: leftTransport, handler: new Handler() });
  const right = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: gateTransport(rightTransport, gate),
    handler: new Handler(),
  });
  const leftDone = left.run();
  const rightDone = right.run();
  await Promise.all([left.ready, right.ready]);
  await Promise.all([left.openAudio(format), right.acceptAudio()]);

  const frame = Uint8Array.from({ length: 320 }, (_, index) => (index % 251) + 1);
  await left.audio.write(frame);

  const closing = right.close();
  for (let attempt = 0; attempt < 500; attempt += 1) {
    const refused = await right.audio.write(new Uint8Array(1)).then(() => false, () => true);
    if (refused) break;
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  openGate();

  assert.deepEqual(await right.audio.read(320), frame, "the frame queued before close was dropped");
  await assert.rejects(
    right.audio.read(1),
    (error: unknown) => error instanceof SessionError && error.code === "closed",
  );
  await closing;
  await Promise.all([leftDone, rightDone]);
});
