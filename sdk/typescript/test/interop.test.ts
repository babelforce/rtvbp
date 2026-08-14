import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline";
import test from "node:test";

import {
  Handler,
  Session,
  babelforceV1,
  classicV1,
  type MediaFormat,
} from "../src/index.ts";
import { NodeWebSocketServer, nodeWebSocketTransport } from "../src/node.ts";

interface PeerCommand {
  readonly name: string;
  readonly command: string;
  readonly arguments: readonly string[];
  readonly cwd: string;
  readonly environment?: NodeJS.ProcessEnv;
}

const peers: readonly PeerCommand[] = [
  {
    name: "Go",
    command: "go",
    arguments: ["run", "."],
    cwd: fileURLToPath(new URL("./interop/go", import.meta.url)),
  },
  {
    name: "Rust",
    command: "cargo",
    arguments: ["run", "--quiet", "--locked", "--manifest-path", "Cargo.toml", "--"],
    cwd: fileURLToPath(new URL("./interop/rust", import.meta.url)),
    environment: {
      ...process.env,
      CARGO_TARGET_DIR: fileURLToPath(new URL("../../rust/target", import.meta.url)),
    },
  },
];

const audioFormat: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

class PeerProcess {
  readonly #child: ChildProcess;
  readonly #stderr: string[] = [];
  readonly #exit: Promise<void>;

  constructor(command: PeerCommand, mode: "server" | "client", argument?: string) {
    this.#child = spawn(
      command.command,
      [...command.arguments, mode, ...(argument === undefined ? [] : [argument])],
      {
        cwd: command.cwd,
        env: command.environment ?? process.env,
        detached: process.platform !== "win32",
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    this.#child.stderr?.on("data", (data: Buffer) => this.#stderr.push(data.toString("utf8")));
    this.#exit = new Promise<void>((resolve, reject) => {
      this.#child.once("error", reject);
      this.#child.once("exit", (code, signal) => {
        if (code === 0) resolve();
        else reject(new Error(
          `interop peer exited with ${code ?? signal ?? "unknown"}: ${this.#stderr.join("")}`,
        ));
      });
    });
    void this.#exit.catch(() => {});
  }

  async firstLine(): Promise<string> {
    const stdout = this.#child.stdout;
    if (stdout === null) throw new Error("interop peer stdout is unavailable");
    const lines = createInterface({ input: stdout });
    return await new Promise<string>((resolve, reject) => {
      const onLine = (line: string): void => {
        lines.off("close", onClose);
        lines.close();
        resolve(line);
      };
      const onClose = (): void => reject(new Error(`interop peer printed no URL: ${this.#stderr.join("")}`));
      lines.once("line", onLine);
      lines.once("close", onClose);
    });
  }

  async wait(): Promise<void> {
    await this.#exit;
  }

  stop(): void {
    if (this.#child.exitCode !== null || this.#child.signalCode !== null) return;
    const pid = this.#child.pid;
    if (process.platform !== "win32" && pid !== undefined) {
      try {
        process.kill(-pid, "SIGTERM");
        return;
      } catch {
        // The group may have exited between the state check and the signal.
      }
    }
    this.#child.kill("SIGTERM");
  }
}

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
  test(`TypeScript client interoperates with ${peer.name} server`, { timeout: 120_000 }, async () => {
    const process = new PeerProcess(peer, "server");
    try {
      const url = await process.firstLine();
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
      await process.wait();
    } finally {
      process.stop();
    }
  });

  test(`${peer.name} client interoperates with TypeScript server`, { timeout: 120_000 }, async () => {
    let sessionResolve: (session: Session) => void = () => {};
    const accepted = new Promise<Session>((resolve) => { sessionResolve = resolve; });
    const server = new NodeWebSocketServer({
      audioFormat,
      createHandler: applicationHandler,
      onSession: sessionResolve,
    });
    await server.listen();
    const process = new PeerProcess(peer, "client", server.url);
    try {
      const session = await accepted;
      await session.ready;
      assertSample(await session.audio.read(320), 1_200);
      await session.audio.write(pcmFrame(-2_400));
      await process.wait();
      while (session.state === "active") await new Promise((resolve) => setTimeout(resolve, 0));
    } finally {
      process.stop();
      await server.close();
    }
  });
}
