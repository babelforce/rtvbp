import assert from "node:assert/strict";
import test from "node:test";

import { Handler, Session, classicV1, demoV1 } from "../src/index.ts";
import { browserWebSocketTransport } from "../src/browser.ts";

class FakeBrowserWebSocket extends EventTarget {
  readonly protocol: string;
  readonly sentBinary: ArrayBuffer[] = [];
  binaryType: BinaryType = "blob";
  bufferedAmount = 0;
  readyState = 0;

  constructor(protocol: string) {
    super();
    this.protocol = protocol;
    queueMicrotask(() => {
      this.readyState = 1;
      this.dispatchEvent(new Event("open"));
    });
  }

  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
    if (typeof data === "string") {
      const frame = classicV1.classicV1Envelope.decode(data);
      if (frame.kind === "request" && frame.method === demoV1.METHOD_DEMO_ECHO) {
        const params = frame.params as { readonly message: string };
        const response = classicV1.classicV1Envelope.encode({
          kind: "response",
          correlationId: frame.id,
          result: { message: params.message },
        });
        queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data: response })));
      }
      return;
    }
    if (data instanceof ArrayBuffer) this.sentBinary.push(data.slice(0));
    else if (ArrayBuffer.isView(data)) {
      this.sentBinary.push(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer);
    }
  }

  close(): void {
    if (this.readyState === 3) return;
    this.readyState = 3;
    queueMicrotask(() => this.dispatchEvent(new Event("close")));
  }
}

test("browser connector uses injected native APIs and contains no Node dependency", async () => {
  let socket: FakeBrowserWebSocket | undefined;
  let offered: readonly string[] | undefined;
  const audioFormat = {
    encoding: "L16",
    sampleRate: 8_000,
    bitDepth: 16,
    channels: 1,
    packetTimeMs: 20,
  } as const;
  const session = new Session({
    envelope: classicV1.classicV1Envelope,
    transportFactory: browserWebSocketTransport({
      url: "wss://example.invalid/rtvbp",
      audioFormat,
      createWebSocket: (_url, protocols) => {
        offered = protocols;
        socket = new FakeBrowserWebSocket(protocols?.[0] ?? "");
        return socket as unknown as WebSocket;
      },
    }),
    handler: new Handler({ adapter: demoV1.voiceAdapter({}, { demoObserved: async () => {} }) }),
  });
  const done = session.run();
  await session.ready;
  assert.deepEqual(offered, ["rtvbp.v1"]);
  assert.deepEqual(
    await new demoV1.ApplicationPeer(session).demoEcho({ message: "browser" }),
    { message: "browser" },
  );

  await session.openAudio(audioFormat);
  const frame = Uint8Array.from({ length: 320 }, (_, index) => (index % 239) + 1);
  await session.audio.write(frame);
  assert.deepEqual(new Uint8Array(socket!.sentBinary[0]!), frame);

  await session.close();
  await done;
  assert.equal(socket?.readyState, 3);
});
