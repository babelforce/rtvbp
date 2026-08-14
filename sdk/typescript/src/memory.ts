import { AsyncQueue } from "./async.ts";
import { SessionError } from "./errors.ts";
import type {
  ControlChannel,
  MediaChannel,
  MediaFormat,
  MediaFrame,
  ReceivedControl,
  Transport,
} from "./transport.ts";
import { mediaFrameBytes } from "./transport.ts";

interface MemoryLink {
  readonly controls: readonly [AsyncQueue<ReceivedControl>, AsyncQueue<ReceivedControl>];
  readonly mediaAccept: readonly [AsyncQueue<MediaChannel>, AsyncQueue<MediaChannel>];
  readonly media: Set<MemoryMediaChannel>;
  closed: boolean;
}

class MemoryControl implements ControlChannel {
  readonly #incoming: AsyncQueue<ReceivedControl>;
  readonly #outgoing: AsyncQueue<ReceivedControl>;
  readonly #link: MemoryLink;

  constructor(incoming: AsyncQueue<ReceivedControl>, outgoing: AsyncQueue<ReceivedControl>, link: MemoryLink) {
    this.#incoming = incoming;
    this.#outgoing = outgoing;
    this.#link = link;
  }

  async send(data: string, signal?: AbortSignal): Promise<void> {
    if (this.#link.closed) throw new SessionError("closed", "memory transport is closed");
    await this.#outgoing.push({ data, receivedAt: Date.now() }, signal);
  }

  async receive(signal?: AbortSignal): Promise<ReceivedControl> {
    return await this.#incoming.shift(signal);
  }
}

class MemoryMediaChannel implements MediaChannel {
  readonly id: string;
  readonly format: MediaFormat;
  readonly #incoming: AsyncQueue<MediaFrame>;
  readonly #outgoing: AsyncQueue<MediaFrame>;
  #closed = false;

  constructor(id: string, format: MediaFormat, incoming: AsyncQueue<MediaFrame>, outgoing: AsyncQueue<MediaFrame>) {
    this.id = id;
    this.format = format;
    this.#incoming = incoming;
    this.#outgoing = outgoing;
  }

  async writeFrame(frame: MediaFrame, signal?: AbortSignal): Promise<void> {
    if (this.#closed) throw new SessionError("closed", "media channel is closed");
    if (frame.data.byteLength !== mediaFrameBytes(this.format)) {
      throw new SessionError("media_frame", "media frame has the wrong byte length");
    }
    await this.#outgoing.push(
      {
        data: frame.data.slice(),
        ...(frame.ptsMs === undefined ? {} : { ptsMs: frame.ptsMs }),
      },
      signal,
    );
  }

  async readFrame(signal?: AbortSignal): Promise<MediaFrame> {
    const frame = await this.#incoming.shift(signal);
    return {
      data: frame.data.slice(),
      ...(frame.ptsMs === undefined ? {} : { ptsMs: frame.ptsMs }),
    };
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#outgoing.close();
    this.#incoming.close();
  }
}

export class MemoryTransport implements Transport {
  readonly control: ControlChannel;
  readonly supportsKeepalive = false;
  readonly #side: 0 | 1;
  readonly #link: MemoryLink;
  readonly #opened = new Set<string>();

  private constructor(side: 0 | 1, link: MemoryLink) {
    this.#side = side;
    this.#link = link;
    this.control = new MemoryControl(
      link.controls[side],
      link.controls[side === 0 ? 1 : 0],
      link,
    );
  }

  static pair(capacity = 64): readonly [MemoryTransport, MemoryTransport] {
    const link: MemoryLink = {
      controls: [new AsyncQueue(capacity), new AsyncQueue(capacity)],
      mediaAccept: [new AsyncQueue(capacity), new AsyncQueue(capacity)],
      media: new Set(),
      closed: false,
    };
    return [new MemoryTransport(0, link), new MemoryTransport(1, link)];
  }

  async openMedia(
    id: string,
    format: MediaFormat,
    signal?: AbortSignal,
  ): Promise<MediaChannel> {
    if (id.length === 0) throw new SessionError("configuration", "media id is required");
    mediaFrameBytes(format);
    if (this.#link.closed) throw new SessionError("closed", "memory transport is closed");
    if (this.#opened.has(id)) throw new SessionError("media_duplicate", `media '${id}' is already open`);
    this.#opened.add(id);
    const left = new AsyncQueue<MediaFrame>(32);
    const right = new AsyncQueue<MediaFrame>(32);
    const local = new MemoryMediaChannel(id, format, left, right);
    const remote = new MemoryMediaChannel(id, format, right, left);
    this.#link.media.add(local);
    this.#link.media.add(remote);
    await this.#link.mediaAccept[this.#side === 0 ? 1 : 0].push(remote, signal);
    return local;
  }

  async acceptMedia(signal?: AbortSignal): Promise<MediaChannel> {
    if (this.#link.closed) throw new SessionError("closed", "memory transport is closed");
    return await this.#link.mediaAccept[this.#side].shift(signal);
  }

  async close(): Promise<void> {
    if (this.#link.closed) return;
    this.#link.closed = true;
    for (const queue of [...this.#link.controls, ...this.#link.mediaAccept]) queue.close();
    await Promise.all([...this.#link.media].map(async (media) => await media.close()));
  }
}
