import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { execFile } from "node:child_process";
import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join, normalize, resolve, sep } from "node:path";
import { createInterface } from "node:readline";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import test, { after, before } from "node:test";

import { chromium, type Browser } from "playwright-core";
import WebSocket, { WebSocketServer } from "ws";

import {
  PROFILE_RTVBP_V1,
  PROFILE_RTVBP_WEBRTC_V1,
} from "../src/generated/zz_generated_profiles.ts";

const execute = promisify(execFile);
const sdkDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");

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
    arguments: ["run", "--quiet", "--locked", "--offline", "--manifest-path", "Cargo.toml", "--"],
    cwd: fileURLToPath(new URL("./interop/rust", import.meta.url)),
    environment: {
      ...process.env,
      CARGO_TARGET_DIR: fileURLToPath(new URL("../../rust/target", import.meta.url)),
    },
  },
];

class PeerProcess {
  readonly #child: ChildProcess;
  readonly #stderr: string[] = [];
  readonly #exit: Promise<void>;

  constructor(command: PeerCommand, binding: "websocket" | "webrtc") {
    this.#child = spawn(command.command, [...command.arguments, "browser-server", binding], {
      cwd: command.cwd,
      env: command.environment ?? process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    this.#child.stderr?.on("data", (data: Buffer) => this.#stderr.push(data.toString("utf8")));
    this.#exit = new Promise<void>((resolveExit, reject) => {
      this.#child.once("error", reject);
      this.#child.once("exit", (code, signal) => {
        if (code === 0) resolveExit();
        else reject(new Error(
          `${command.name} browser peer exited with ${code ?? signal ?? "unknown"}: ${this.#stderr.join("")}`,
        ));
      });
    });
    void this.#exit.catch(() => {});
  }

  async firstLine(): Promise<string> {
    const stdout = this.#child.stdout;
    if (stdout === null) throw new Error("browser interop peer stdout is unavailable");
    const lines = createInterface({ input: stdout });
    return await new Promise<string>((resolveLine, reject) => {
      const onLine = (line: string): void => {
        lines.off("close", onClose);
        lines.close();
        resolveLine(line);
      };
      const onClose = (): void => reject(new Error(`browser interop peer printed no URL: ${this.#stderr.join("")}`));
      lines.once("line", onLine);
      lines.once("close", onClose);
    });
  }

  async wait(): Promise<void> {
    await this.#exit;
  }

  diagnostic(): string {
    return this.#stderr.join("");
  }

  stop(): void {
    if (this.#child.exitCode === null && this.#child.signalCode === null) this.#child.kill("SIGTERM");
  }
}

function contentType(path: string): string {
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".map")) return "application/json; charset=utf-8";
  return "application/octet-stream";
}

class BrowserOriginProxy {
  readonly #upstreamUrl: string;
  readonly #profile: string;
  readonly #server: Server;
  readonly #websockets: WebSocketServer;
  readonly #upstreams = new Set<WebSocket>();
  #origin = "";

  constructor(upstreamUrl: string, profile: string) {
    this.#upstreamUrl = upstreamUrl;
    this.#profile = profile;
    this.#server = createServer((request, response) => { void this.#serve(request.url ?? "/", response); });
    this.#websockets = new WebSocketServer({
      noServer: true,
      handleProtocols: (protocols) => protocols.has(this.#profile) ? this.#profile : false,
    });
    this.#server.on("upgrade", (request, socket, head) => {
      if (request.url !== "/rtvbp") {
        socket.destroy();
        return;
      }
      this.#websockets.handleUpgrade(request, socket, head, (client) => {
        this.#websockets.emit("connection", client, request);
      });
    });
    this.#websockets.on("connection", (client) => this.#bridge(client));
  }

  get origin(): string {
    return this.#origin;
  }

  get websocketUrl(): string {
    return this.#origin.replace(/^http/, "ws") + "/rtvbp";
  }

  async listen(): Promise<void> {
    await new Promise<void>((resolveListen, reject) => {
      this.#server.once("error", reject);
      this.#server.listen(0, "127.0.0.1", () => resolveListen());
    });
    const address = this.#server.address();
    if (address === null || typeof address === "string") throw new Error("browser proxy address unavailable");
    this.#origin = `http://127.0.0.1:${address.port}`;
  }

  async close(): Promise<void> {
    for (const socket of this.#upstreams) socket.terminate();
    for (const client of this.#websockets.clients) client.terminate();
    await new Promise<void>((resolveClose) => this.#websockets.close(() => resolveClose()));
    await new Promise<void>((resolveClose, reject) => this.#server.close((error) => error === undefined ? resolveClose() : reject(error)));
  }

  async #serve(url: string, response: import("node:http").ServerResponse): Promise<void> {
    if (url === "/" || url === "/index.html") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(`<!doctype html><meta charset="utf-8"><title>RTVBP browser proof</title>
        <script type="importmap">{"imports":{"lossless-json":"/vendor/index.js"}}</script>`);
      return;
    }
    const route = url.split("?", 1)[0] ?? "";
    let root: string;
    let relative: string;
    if (route.startsWith("/dist/")) {
      root = resolve(sdkDirectory, "dist");
      relative = route.slice("/dist/".length);
    } else if (route.startsWith("/vendor/")) {
      root = resolve(sdkDirectory, "node_modules/lossless-json/lib/esm");
      relative = route.slice("/vendor/".length);
    } else {
      response.writeHead(404).end();
      return;
    }
    const path = normalize(resolve(root, relative));
    if (path !== root && !path.startsWith(root + sep)) {
      response.writeHead(400).end();
      return;
    }
    try {
      const body = await readFile(path);
      response.writeHead(200, { "content-type": contentType(path), "cache-control": "no-store" });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  }

  #bridge(client: WebSocket): void {
    const upstream = new WebSocket(this.#upstreamUrl, [this.#profile]);
    this.#upstreams.add(upstream);
    const pending: { readonly data: WebSocket.RawData; readonly binary: boolean }[] = [];
    let pendingBytes = 0;
    client.on("message", (data, binary) => {
      if (upstream.readyState === WebSocket.OPEN) {
        upstream.send(data, { binary });
        return;
      }
      pendingBytes += Array.isArray(data)
        ? data.reduce((total, part) => total + part.byteLength, 0)
        : data.byteLength;
      if (pending.length >= 256 || pendingBytes > 2 * 1024 * 1024) {
        client.close(1011, "proxy queue full");
        return;
      }
      pending.push({ data, binary });
    });
    upstream.once("open", () => {
      for (const message of pending.splice(0)) upstream.send(message.data, { binary: message.binary });
    });
    upstream.on("message", (data, binary) => {
      if (client.readyState === WebSocket.OPEN) client.send(data, { binary });
    });
    const closeUpstream = (): void => {
      if (upstream.readyState === WebSocket.OPEN || upstream.readyState === WebSocket.CONNECTING) upstream.close();
    };
    const closeClient = (): void => {
      this.#upstreams.delete(upstream);
      if (client.readyState === WebSocket.OPEN || client.readyState === WebSocket.CONNECTING) client.close();
    };
    client.once("close", closeUpstream);
    upstream.once("close", closeClient);
    upstream.once("error", () => client.close(1011, "upstream failed"));
  }
}

function wavTone(seconds: number): Buffer {
  const rate = 48_000;
  const samples = rate * seconds;
  const dataBytes = samples * 2;
  const result = Buffer.alloc(44 + dataBytes);
  result.write("RIFF", 0);
  result.writeUInt32LE(36 + dataBytes, 4);
  result.write("WAVEfmt ", 8);
  result.writeUInt32LE(16, 16);
  result.writeUInt16LE(1, 20);
  result.writeUInt16LE(1, 22);
  result.writeUInt32LE(rate, 24);
  result.writeUInt32LE(rate * 2, 28);
  result.writeUInt16LE(2, 32);
  result.writeUInt16LE(16, 34);
  result.write("data", 36);
  result.writeUInt32LE(dataBytes, 40);
  for (let index = 0; index < samples; index += 1) {
    result.writeInt16LE(Math.round(Math.sin(index * 2 * Math.PI * 440 / rate) * 12_000), 44 + index * 2);
  }
  return result;
}

let browser: Browser;
let temporaryDirectory = "";

async function browserExecutable(): Promise<string> {
  const candidates = [
    process.env.RTVBP_BROWSER,
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter((candidate): candidate is string => candidate !== undefined && candidate.length > 0);
  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Try the next explicit public browser executable location.
    }
  }
  throw new Error("Chrome or Chromium is required for the real-browser compatibility gate; set RTVBP_BROWSER");
}

before(async () => {
  await execute("npm", ["run", "build"], { cwd: sdkDirectory });
  temporaryDirectory = await mkdtemp(join(tmpdir(), "rtvbp-browser-proof-"));
  const microphone = join(temporaryDirectory, "microphone.wav");
  await writeFile(microphone, wavTone(20));
  const realDevice = process.env.RTVBP_REAL_DEVICE_SMOKE === "1";
  browser = await chromium.launch({
    headless: true,
    executablePath: await browserExecutable(),
    ignoreDefaultArgs: ["--mute-audio"],
    args: [
      "--no-sandbox",
      "--disable-dev-shm-usage",
      "--autoplay-policy=no-user-gesture-required",
      "--use-fake-ui-for-media-stream",
      ...(realDevice ? [] : [
        "--use-fake-device-for-media-stream",
        `--use-file-for-fake-audio-capture=${microphone}`,
      ]),
    ],
  });
});

after(async () => {
  await browser?.close();
  if (temporaryDirectory.length > 0) await rm(temporaryDirectory, { recursive: true, force: true });
});

interface BrowserResult {
  readonly sessionState: string;
  readonly deviceState: string;
  readonly clearCalls: number;
  readonly speechStartedCalls: number;
  readonly clearBytes: number;
  readonly capturedSdkSamples: number;
  readonly playedSdkSamples: number;
  readonly inboundPackets: number;
  readonly inboundAudioEnergy: number;
  readonly renderedAudioEnergy: number;
  readonly remoteTrackAttached: boolean;
  readonly outboundPackets: number;
  readonly microphoneEnded: boolean;
  readonly pingRoundTrip: boolean;
}

async function runBrowserCase(
  origin: string,
  websocketUrl: string,
  profile: string,
): Promise<BrowserResult> {
  const context = await browser.newContext();
  await context.grantPermissions(["microphone"], { origin });
  const page = await context.newPage();
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.goto(origin, { waitUntil: "domcontentloaded" });
    const result = await page.evaluate(async ({ selectedProfile, url }) => {
      const corePath: string = "/dist/index.js";
      const browserPath: string = "/dist/browser.js";
      const core = await import(corePath) as typeof import("../src/index.ts");
      const browserSdk = await import(browserPath) as typeof import("../src/browser.ts");
      const audioFormat = core.profileMediaFormat(selectedProfile, "audio");
      let microphone: MediaStream | undefined;
      let clearCalls = 0;
      let speechStartedCalls = 0;
      let clearBytes = 0;
      let connectedTransport: import("../src/transport.ts").Transport | undefined;
      const nativeGetUserMedia = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
      const device = new browserSdk.BrowserAudioDevice({
        playbackBufferMs: 1_000,
        getUserMedia: async (constraints: MediaStreamConstraints) => {
          microphone = await nativeGetUserMedia(constraints);
          return microphone;
        },
      });
      const requests = new Proxy({}, {
        get: (_target, property) => {
          if (property === "audioBufferClear") {
            return async () => {
              clearCalls += 1;
              clearBytes += device.clearPlayback();
              return { len: clearBytes };
            };
          }
          return async () => ({});
        },
      }) as import("../src/generated/zz_generated_babelforcev1_roles.ts").VoiceHandler;
      const events = new Proxy({}, {
        get: (_target, property) => property === "audioSpeechStarted"
          ? async () => { speechStartedCalls += 1; }
          : async () => {},
      }) as
        import("../src/generated/zz_generated_babelforcev1_roles.ts").VoiceEventHandler;
      const isWebRtc = selectedProfile === core.profiles.PROFILE_RTVBP_WEBRTC_V1;
      const handler = new core.Handler({
        adapter: core.babelforceV1.voiceAdapter(requests, events),
        onBegin: async (handlerContext) => {
          await handlerContext.acceptAudio();
          if (!isWebRtc) await device.attachWebSocket(handlerContext.audio);
        },
      });
      const baseFactory = isWebRtc
        ? browserSdk.browserWebRtcTransport({ url, audioDevice: device })
        : browserSdk.browserWebSocketTransport({ url, protocols: [selectedProfile], audioFormat });
      const factory = async (
        envelope: import("../src/envelope.ts").EnvelopeCodec,
        signal: AbortSignal,
      ) => {
        connectedTransport = await baseFactory(envelope, signal);
        return connectedTransport;
      };
      const session = new core.Session({
        envelope: core.classicV1.classicV1Envelope,
        transportFactory: factory,
        handler,
      });
      const done = session.run();
      await session.ready;

      const deadline = Date.now() + 15_000;
      let inboundPackets = 0;
      let inboundAudioEnergy = 0;
      let outboundPackets = 0;
      while (Date.now() < deadline) {
        if (isWebRtc) {
          const report = await (connectedTransport as import("../src/browser_webrtc.ts").BrowserWebRtcTransport).getStats();
          report.forEach((stat) => {
            const mediaKind = stat.kind ?? stat.mediaType;
            if (mediaKind !== "audio") return;
            if (stat.type === "inbound-rtp") {
              inboundPackets = Math.max(inboundPackets, stat.packetsReceived ?? 0);
              inboundAudioEnergy = Math.max(inboundAudioEnergy, stat.totalAudioEnergy ?? 0);
            }
            if (stat.type === "outbound-rtp") outboundPackets = Math.max(outboundPackets, stat.packetsSent ?? 0);
          });
        } else {
          inboundPackets = device.stats.playbackFrames;
          outboundPackets = device.stats.captureFrames;
        }
        if (
          clearCalls > 0
          && speechStartedCalls > 0
          && inboundPackets > 0
          && outboundPackets > 0
          && (!isWebRtc || device.stats.remoteTrackAttached)
        ) break;
        await new Promise((resolveWait) => setTimeout(resolveWait, 20));
      }
      if (
        clearCalls === 0
        || speechStartedCalls === 0
        || inboundPackets === 0
        || outboundPackets === 0
        || (isWebRtc && !device.stats.remoteTrackAttached)
      ) {
        throw new Error(
          `incomplete media evidence clear=${clearCalls} in=${inboundPackets} out=${outboundPackets} energy=${inboundAudioEnergy}`,
        );
      }
      const request = { t0: Date.now(), data: { proof: selectedProfile } };
      const response = await new core.babelforceV1.ApplicationPeer(session).ping(request);
      const pingRoundTrip = response.t0 === request.t0;
      const deviceStats = device.stats;
      await new core.babelforceV1.ApplicationPeer(session).sessionTerminate({ reason: "browser proof complete" });
      await done;
      await device.close();
      return {
        sessionState: session.state,
        deviceState: device.state,
        clearCalls,
        speechStartedCalls,
        clearBytes,
        capturedSdkSamples: deviceStats.capturedSdkSamples,
        playedSdkSamples: deviceStats.playedSdkSamples,
        inboundPackets,
        inboundAudioEnergy,
        renderedAudioEnergy: deviceStats.remoteAudioEnergy,
        remoteTrackAttached: deviceStats.remoteTrackAttached,
        outboundPackets,
        microphoneEnded: microphone?.getTracks().every((track) => track.readyState === "ended") ?? false,
        pingRoundTrip,
      };
    }, { selectedProfile: profile, url: websocketUrl }) as BrowserResult;
    assert.deepEqual(pageErrors, []);
    return result;
  } finally {
    await context.close();
  }
}

for (const peer of peers) {
  for (const binding of ["websocket", "webrtc"] as const) {
    const profile = binding === "websocket" ? PROFILE_RTVBP_V1 : PROFILE_RTVBP_WEBRTC_V1;
    test(`real browser ${binding} audio interoperates with ${peer.name}`, { timeout: 90_000 }, async () => {
      const process = new PeerProcess(peer, binding);
      let proxy: BrowserOriginProxy | undefined;
      try {
        const upstream = await process.firstLine();
        assert.match(upstream, /^ws:\/\/127\.0\.0\.1:/);
        proxy = new BrowserOriginProxy(upstream, profile);
        await proxy.listen();
        const result = await runBrowserCase(proxy.origin, proxy.websocketUrl, profile);
        assert.equal(result.sessionState, "closed");
        assert.equal(result.deviceState, "closed");
        assert.equal(result.clearCalls, 1);
        assert.equal(result.speechStartedCalls, 1);
        assert.ok(result.inboundPackets > 0, "browser must receive non-silent audio packets");
        assert.ok(result.outboundPackets > 0, "browser must send non-silent audio packets");
        assert.equal(result.microphoneEnded, true);
        assert.equal(result.pingRoundTrip, true);
        if (binding === "websocket") {
          assert.ok(result.clearBytes > 0, "WebSocket barge-in must drop buffered L16 bytes");
          assert.ok(result.capturedSdkSamples > 0);
          assert.ok(result.playedSdkSamples > 0);
        } else {
          assert.equal(result.remoteTrackAttached, true, "browser must attach the remote WebRTC track for rendering");
        }
        await process.wait();
      } catch (error) {
        const diagnostic = process.diagnostic();
        throw new Error(
          `${error instanceof Error ? error.message : String(error)}${diagnostic.length === 0 ? "" : `; peer: ${diagnostic}`}`,
          { cause: error },
        );
      } finally {
        process.stop();
        await proxy?.close();
      }
    });
  }
}

test("real browser surfaces permission failure and closes the adapter", { timeout: 30_000 }, async () => {
  const target = new WebSocketServer({ port: 0 });
  await new Promise<void>((resolveListening) => target.once("listening", () => resolveListening()));
  const address = target.address();
  if (address === null || typeof address === "string") throw new Error("test WebSocket address unavailable");
  const proxy = new BrowserOriginProxy(`ws://127.0.0.1:${address.port}`, PROFILE_RTVBP_V1);
  await proxy.listen();
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    await page.goto(proxy.origin);
    const result = await page.evaluate(async () => {
      const browserPath: string = "/dist/browser.js";
      const { BrowserAudioDevice } = await import(browserPath) as typeof import("../src/browser.ts");
      const device = new BrowserAudioDevice({
        getUserMedia: async () => { throw new DOMException("permission denied", "NotAllowedError"); },
      });
      let code = "";
      try {
        await device.prepareWebRtc();
      } catch (error) {
        code = (error as { readonly code?: string }).code ?? "";
      }
      await device.close();
      return { code, state: device.state };
    });
    assert.deepEqual(result, { code: "media_permission", state: "closed" });
  } finally {
    await context.close();
    await proxy.close();
    await new Promise<void>((resolveClose) => target.close(() => resolveClose()));
  }
});

test("real browser cancellation settles the session and releases microphone and AudioContext", { timeout: 30_000 }, async () => {
  const target = new WebSocketServer({
    port: 0,
    handleProtocols: (protocols) => protocols.has(PROFILE_RTVBP_V1) ? PROFILE_RTVBP_V1 : false,
  });
  target.on("connection", (socket) => socket.on("message", () => {}));
  await new Promise<void>((resolveListening) => target.once("listening", () => resolveListening()));
  const address = target.address();
  if (address === null || typeof address === "string") throw new Error("test WebSocket address unavailable");
  const proxy = new BrowserOriginProxy(`ws://127.0.0.1:${address.port}`, PROFILE_RTVBP_V1);
  await proxy.listen();
  const context = await browser.newContext();
  await context.grantPermissions(["microphone"], { origin: proxy.origin });
  const page = await context.newPage();
  try {
    await page.goto(proxy.origin);
    const result = await page.evaluate(async ({ url }) => {
      const corePath: string = "/dist/index.js";
      const browserPath: string = "/dist/browser.js";
      const core = await import(corePath) as typeof import("../src/index.ts");
      const browserSdk = await import(browserPath) as typeof import("../src/browser.ts");
      let stream: MediaStream | undefined;
      const nativeGetUserMedia = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
      const device = new browserSdk.BrowserAudioDevice({
        getUserMedia: async (constraints: MediaStreamConstraints) => {
          stream = await nativeGetUserMedia(constraints);
          return stream;
        },
      });
      const format = core.profileMediaFormat(core.profiles.PROFILE_RTVBP_V1, "audio");
      const session = new core.Session({
        envelope: core.classicV1.classicV1Envelope,
        transportFactory: browserSdk.browserWebSocketTransport({ url, audioFormat: format }),
        handler: new core.Handler({
          onBegin: async (handlerContext) => {
            await handlerContext.openAudio(format);
            await device.attachWebSocket(handlerContext.audio);
          },
        }),
      });
      const abort = new AbortController();
      const done = session.run(abort.signal);
      await session.ready;
      const deadline = Date.now() + 5_000;
      while (device.stats.captureFrames === 0 && Date.now() < deadline) {
        await new Promise((resolveWait) => setTimeout(resolveWait, 20));
      }
      abort.abort("test cancellation");
      await done;
      await device.close();
      return {
        session: session.state,
        device: device.state,
        captureFrames: device.stats.captureFrames,
        tracksEnded: stream?.getTracks().every((track) => track.readyState === "ended") ?? false,
      };
    }, { url: proxy.websocketUrl });
    assert.equal(result.session, "closed");
    assert.equal(result.device, "closed");
    assert.ok(result.captureFrames > 0);
    assert.equal(result.tracksEnded, true);
  } finally {
    await context.close();
    await proxy.close();
    await new Promise<void>((resolveClose) => target.close(() => resolveClose()));
  }
});

test(
  "bounded real-device smoke exchanges WebRTC microphone and speaker media",
  {
    timeout: 90_000,
    skip: process.env.RTVBP_REAL_DEVICE_SMOKE !== "1" ? "set RTVBP_REAL_DEVICE_SMOKE=1 explicitly" : false,
  },
  async () => {
    const process = new PeerProcess(peers[0]!, "webrtc");
    let proxy: BrowserOriginProxy | undefined;
    try {
      const upstream = await process.firstLine();
      proxy = new BrowserOriginProxy(upstream, PROFILE_RTVBP_WEBRTC_V1);
      await proxy.listen();
      const result = await runBrowserCase(proxy.origin, proxy.websocketUrl, PROFILE_RTVBP_WEBRTC_V1);
      assert.equal(result.sessionState, "closed");
      assert.equal(result.deviceState, "closed");
      assert.equal(result.remoteTrackAttached, true);
      assert.ok(result.inboundPackets > 0);
      assert.ok(result.outboundPackets > 0);
      assert.equal(result.microphoneEnded, true);
      await process.wait();
    } finally {
      process.stop();
      await proxy?.close();
    }
  },
);
