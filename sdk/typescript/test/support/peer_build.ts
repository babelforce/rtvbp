import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import type { PeerCommand } from "./peer_process.ts";

const goDirectory = fileURLToPath(new URL("../interop/go", import.meta.url));
const rustDirectory = fileURLToPath(new URL("../interop/rust", import.meta.url));
const rustTargetDirectory = fileURLToPath(new URL("../../../rust/target", import.meta.url));

export interface BuiltPeers {
  readonly go: PeerCommand;
  readonly rust: PeerCommand;
  dispose(): Promise<void>;
}

async function run(command: string, args: readonly string[], cwd: string, label: string): Promise<void> {
  const child = spawn(command, [...args], {
    cwd,
    env: { ...process.env, CARGO_TARGET_DIR: rustTargetDirectory },
    stdio: ["ignore", "ignore", "pipe"],
  });
  const stderr: string[] = [];
  child.stderr.on("data", (data: Buffer) => stderr.push(data.toString("utf8")));
  await new Promise<void>((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${label} exited with ${code}: ${stderr.join("")}`));
    });
  });
}

/**
 * Compiles the Go and Rust interop peers once, in a `before` hook, so no test spends its budget
 * on a compiler (or on the slot queue in front of it) and no test starts `go run` or `cargo run`,
 * whose compiled child outlives the parent when a test is abandoned. Both builds run together.
 */
export async function buildPeers(): Promise<BuiltPeers> {
  const directory = await mkdtemp(join(tmpdir(), "rtvbp-interop-"));
  const goBinary = join(directory, "rtvbp-interop-go");
  try {
    await Promise.all([
      run("go", ["build", "-o", goBinary, "."], goDirectory, "go build of the interop peer"),
      run(
        "cargo",
        ["build", "--quiet", "--locked", "--manifest-path", "Cargo.toml"],
        rustDirectory,
        "cargo build of the interop peer",
      ),
    ]);
  } catch (error) {
    await rm(directory, { recursive: true, force: true });
    throw error;
  }
  return {
    go: { name: "Go", command: goBinary, arguments: [], cwd: goDirectory },
    rust: {
      name: "Rust",
      command: join(rustTargetDirectory, "debug", "rtvbp-typescript-interop-rust"),
      arguments: [],
      cwd: rustDirectory,
    },
    dispose: async () => await rm(directory, { recursive: true, force: true }),
  };
}
