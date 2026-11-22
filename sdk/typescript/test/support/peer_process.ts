import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import type { TestContext } from "node:test";

export interface PeerCommand {
  readonly name: string;
  readonly command: string;
  readonly arguments: readonly string[];
  readonly cwd: string;
  readonly environment?: NodeJS.ProcessEnv;
}

const live = new Set<PeerProcess>();

/** Kills every peer still running, with its whole process group. Safe to call twice. */
export function stopAllPeers(): void {
  for (const peer of [...live]) peer.stop();
}

// A test that times out is abandoned, not unwound: its `finally` never runs while the body is
// still awaiting. This exit hook is the last line behind `startPeer`'s per-test cleanup.
process.once("exit", stopAllPeers);

export class PeerProcess {
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
    live.add(this);
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

  /**
   * Kills the peer and everything it started. `go run` and `cargo run` leave a compiled child that
   * outlives its parent, so the signal goes to the process group, and it is SIGKILL: a peer that
   * is stopped has no state worth a graceful exit. The group is signalled even when the leader
   * has already exited, since the child can outlive it.
   */
  stop(): void {
    live.delete(this);
    const pid = this.#child.pid;
    if (process.platform !== "win32" && pid !== undefined) {
      try {
        process.kill(-pid, "SIGKILL");
        return;
      } catch {
        // The group is already gone.
      }
    }
    if (this.#child.exitCode === null && this.#child.signalCode === null) {
      this.#child.kill("SIGKILL");
    }
  }
}

/** Starts a peer that is stopped when the test ends, whether it passed, failed or timed out. */
export function startPeer(
  context: TestContext,
  command: PeerCommand,
  mode: "server" | "client",
  argument?: string,
): PeerProcess {
  const peer = new PeerProcess(command, mode, argument);
  context.after(() => peer.stop());
  return peer;
}
