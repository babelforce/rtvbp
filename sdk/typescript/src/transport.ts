import type { EnvelopeCodec } from "./envelope.ts";
import { SessionError } from "./errors.ts";

export interface ReceivedControl {
  readonly data: string;
  readonly receivedAt: number;
}

export interface MediaFormat {
  readonly encoding: "L16";
  readonly sampleRate: number;
  readonly bitDepth: 16;
  readonly channels: number;
  readonly packetTimeMs: number;
}

export interface MediaFrame {
  readonly data: Uint8Array;
  readonly ptsMs?: number;
}

export interface KeepalivePolicy {
  readonly intervalMs: number;
  readonly timeoutMs: number;
  readonly maxMisses: number;
}

export interface ControlChannel {
  send(data: string, signal?: AbortSignal): Promise<void>;
  receive(signal?: AbortSignal): Promise<ReceivedControl>;
}

export interface MediaChannel {
  readonly id: string;
  readonly format: MediaFormat;
  writeFrame(frame: MediaFrame, signal?: AbortSignal): Promise<void>;
  readFrame(signal?: AbortSignal): Promise<MediaFrame>;
  close(): Promise<void>;
}

export interface Transport {
  readonly control: ControlChannel;
  openMedia(id: string, format: MediaFormat, signal?: AbortSignal): Promise<MediaChannel>;
  acceptMedia(signal?: AbortSignal): Promise<MediaChannel>;
  close(): Promise<void>;
  readonly supportsKeepalive?: boolean;
  monitorKeepalive?(policy: KeepalivePolicy, signal: AbortSignal): Promise<void>;
}

export type TransportFactory = (
  envelope: EnvelopeCodec,
  signal: AbortSignal,
) => Promise<Transport>;

export function mediaFrameBytes(format: MediaFormat): number {
  if (format.encoding !== "L16") throw new SessionError("media_format", "encoding must be L16");
  for (const [name, value] of [
    ["sample rate", format.sampleRate],
    ["channel count", format.channels],
    ["packet time", format.packetTimeMs],
  ] as const) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new SessionError("media_format", `${name} must be a positive safe integer`);
    }
  }
  if (format.bitDepth !== 16) throw new SessionError("media_format", "L16 bit depth must be 16");
  const samples = (format.sampleRate * format.packetTimeMs) / 1000;
  if (!Number.isSafeInteger(samples) || samples <= 0) {
    throw new SessionError("media_format", "packet time must contain a whole sample count");
  }
  const bytes = samples * format.channels * 2;
  if (!Number.isSafeInteger(bytes) || bytes <= 0) {
    throw new SessionError("media_format", "frame byte count is out of range");
  }
  return bytes;
}

export function validateKeepalive(policy: KeepalivePolicy | undefined): void {
  if (policy === undefined) return;
  for (const [name, value] of [
    ["interval", policy.intervalMs],
    ["timeout", policy.timeoutMs],
    ["max misses", policy.maxMisses],
  ] as const) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new SessionError("configuration", `keepalive ${name} must be positive`);
    }
  }
}
