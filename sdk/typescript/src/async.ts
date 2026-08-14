import { SessionError, aborted, throwIfAborted } from "./errors.ts";

interface Waiter<T> {
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
  readonly signal?: AbortSignal;
  onAbort?: () => void;
}

interface Writer<T> extends Waiter<void> {
  readonly value: T;
}

function detach<T>(waiter: Waiter<T>): void {
  if (waiter.signal !== undefined && waiter.onAbort !== undefined) {
    waiter.signal.removeEventListener("abort", waiter.onAbort);
  }
}

/** A bounded, drain-on-close asynchronous queue used by every in-memory channel. */
export class AsyncQueue<T> {
  readonly #capacity: number;
  readonly #items: T[] = [];
  readonly #readers: Waiter<T>[] = [];
  readonly #writers: Writer<T>[] = [];
  #closed = false;

  constructor(capacity: number) {
    if (!Number.isSafeInteger(capacity) || capacity <= 0) {
      throw new SessionError("configuration", "queue capacity must be a positive safe integer");
    }
    this.#capacity = capacity;
  }

  get length(): number {
    return this.#items.length;
  }

  /** Admit immediately or report bounded-capacity exhaustion without creating a waiter. */
  tryPush(value: T): boolean {
    if (this.#closed) return false;
    const reader = this.#readers.shift();
    if (reader !== undefined) {
      detach(reader);
      reader.resolve(value);
      return true;
    }
    if (this.#items.length >= this.#capacity) return false;
    this.#items.push(value);
    return true;
  }

  async push(value: T, signal?: AbortSignal): Promise<void> {
    throwIfAborted(signal);
    if (this.#closed) throw new SessionError("closed", "queue is closed");
    const reader = this.#readers.shift();
    if (reader !== undefined) {
      detach(reader);
      reader.resolve(value);
      return;
    }
    if (this.#items.length < this.#capacity) {
      this.#items.push(value);
      return;
    }
    await new Promise<void>((resolve, reject) => {
      const writer: Writer<T> = {
        value,
        resolve,
        reject,
        ...(signal === undefined ? {} : { signal }),
      };
      if (signal !== undefined) {
        writer.onAbort = () => {
          const index = this.#writers.indexOf(writer);
          if (index >= 0) this.#writers.splice(index, 1);
          reject(aborted(signal));
        };
        signal.addEventListener("abort", writer.onAbort, { once: true });
      }
      this.#writers.push(writer);
    });
  }

  async shift(signal?: AbortSignal): Promise<T> {
    throwIfAborted(signal);
    if (this.#items.length > 0) {
      const value = this.#items.shift() as T;
      this.#admitWriter();
      return value;
    }
    const writer = this.#writers.shift();
    if (writer !== undefined) {
      detach(writer);
      writer.resolve();
      return writer.value;
    }
    if (this.#closed) throw new SessionError("closed", "queue is closed");
    return await new Promise<T>((resolve, reject) => {
      const reader: Waiter<T> = {
        resolve,
        reject,
        ...(signal === undefined ? {} : { signal }),
      };
      if (signal !== undefined) {
        reader.onAbort = () => {
          const index = this.#readers.indexOf(reader);
          if (index >= 0) this.#readers.splice(index, 1);
          reject(aborted(signal));
        };
        signal.addEventListener("abort", reader.onAbort, { once: true });
      }
      this.#readers.push(reader);
    });
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    const error = new SessionError("closed", "queue is closed");
    for (const writer of this.#writers.splice(0)) {
      detach(writer);
      writer.reject(error);
    }
    if (this.#items.length === 0) {
      for (const reader of this.#readers.splice(0)) {
        detach(reader);
        reader.reject(error);
      }
    }
  }

  clear(): void {
    this.#items.length = 0;
    while (this.#writers.length > 0 && !this.#closed) this.#admitWriter();
    if (this.#closed) this.#closeReaders();
  }

  #admitWriter(): void {
    const writer = this.#writers.shift();
    if (writer === undefined) {
      if (this.#closed && this.#items.length === 0) this.#closeReaders();
      return;
    }
    detach(writer);
    const reader = this.#readers.shift();
    if (reader !== undefined) {
      detach(reader);
      reader.resolve(writer.value);
    } else {
      this.#items.push(writer.value);
    }
    writer.resolve();
  }

  #closeReaders(): void {
    const error = new SessionError("closed", "queue is closed");
    for (const reader of this.#readers.splice(0)) {
      detach(reader);
      reader.reject(error);
    }
  }
}

export async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  code: string,
  message: string,
  signal?: AbortSignal,
): Promise<T> {
  throwIfAborted(signal);
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new SessionError("configuration", "timeout must be positive");
  }
  return await new Promise<T>((resolve, reject) => {
    let settled = false;
    let timer: ReturnType<typeof setTimeout>;
    const finish = (callback: () => void): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      callback();
    };
    timer = setTimeout(() => finish(() => reject(new SessionError(code, message))), timeoutMs);
    const onAbort = (): void => finish(() => reject(aborted(signal)));
    signal?.addEventListener("abort", onAbort, { once: true });
    promise.then(
      (value) => finish(() => resolve(value)),
      (error: unknown) => finish(() => reject(error)),
    );
  });
}
