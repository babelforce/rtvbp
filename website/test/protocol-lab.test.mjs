import assert from "node:assert/strict";
import {access} from "node:fs/promises";
import {spawn} from "node:child_process";
import test from "node:test";

import {chromium} from "playwright-core";

const port = 31847;
const origin = `http://127.0.0.1:${port}`;

async function browserExecutable() {
  const candidates = [
    process.env.RTVBP_BROWSER,
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);
  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Try the next system browser.
    }
  }
  throw new Error("set RTVBP_BROWSER to a Chromium-compatible executable");
}

async function waitForServer(process) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    if (process.exitCode !== null) throw new Error(`docs server exited with ${process.exitCode}`);
    try {
      const response = await fetch(`${origin}/rtvbp/try`);
      if (response.ok) return;
    } catch {
      // Server is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("timed out waiting for the docs server");
}

test("landing CTA opens a reusable, accessible browser protocol lab", {timeout: 60_000}, async () => {
  const server = spawn(
    process.execPath,
    [
      "node_modules/@docusaurus/core/bin/docusaurus.mjs",
      "serve",
      "--host",
      "127.0.0.1",
      "--port",
      String(port),
      "--no-open",
    ],
    {stdio: ["ignore", "pipe", "pipe"]},
  );
  let serverOutput = "";
  server.stdout.on("data", (chunk) => { serverOutput += chunk; });
  server.stderr.on("data", (chunk) => { serverOutput += chunk; });

  let browser;
  try {
    await waitForServer(server);
    browser = await chromium.launch({
      executablePath: await browserExecutable(),
      headless: true,
      ignoreDefaultArgs: ["--mute-audio"],
      args: [
        "--no-sandbox",
        "--autoplay-policy=no-user-gesture-required",
        "--use-fake-ui-for-media-stream",
        "--use-fake-device-for-media-stream",
      ],
    });
    const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    await page.goto(`${origin}/rtvbp/`);
    const cta = page.locator("a.button--primary", {hasText: "Try it out"});
    await cta.waitFor();
    await cta.click();
    await page.waitForURL("**/rtvbp/try");

    await page.getByRole("heading", {name: "See a voice session happen."}).waitFor();
    await page.getByRole("button", {name: "Call"}).click();
    await page.getByRole("status").getByText("Connected", {exact: true}).waitFor({timeout: 15_000});
    await page.getByText("session.initialize", {exact: true}).first().waitFor();
    await page.getByText("rtvbp.webrtc.v1", {exact: true}).first().waitFor();
    await page.waitForFunction(() => {
      const meter = document.querySelector('[data-testid="audio-meter-voice"]');
      return meter !== null && Number.parseFloat(getComputedStyle(meter).width) > 0;
    });
    await page.getByText("PCMU / 8 kHz", {exact: true}).waitFor();

    await page.getByRole("button", {name: "Mute"}).click();
    await assert.doesNotReject(async () => {
      await page.getByRole("button", {name: "Unmute"}).waitFor();
    });
    await page.getByRole("button", {name: "Send DTMF 5"}).click();
    await page.getByText("dtmf", {exact: true}).first().waitFor();
    await page.getByRole("button", {name: "Barge in"}).click();
    await page.getByText("audio.speech.started", {exact: true}).first().waitFor();
    await page.getByRole("button", {name: "Clear audio buffer"}).click();
    await page.getByText("audio.buffer.clear", {exact: true}).first().waitFor();

    await page.getByRole("button", {name: "Show raw payloads"}).click();
    await page.getByText(/"version":"1"/).first().waitFor();
    await page.getByRole("button", {name: "Next scenario step"}).click();
    await page.getByText("Generated conformance scenario", {exact: true}).waitFor();

    await page.getByRole("button", {name: "Hang up"}).click();
    await page.getByText("Ended", {exact: true}).waitFor();
    await page.getByRole("button", {name: "Call again"}).click();
    await page.getByRole("status").getByText("Connected", {exact: true}).waitFor({timeout: 15_000});
    await page.getByRole("button", {name: "Hang up"}).click();

    await page.getByRole("radio", {name: /WebSocket/}).check();
    await page.getByRole("button", {name: "Call again"}).click();
    await page.getByRole("status").getByText("Connected", {exact: true}).waitFor();
    await page.getByText("L16 / 8 kHz", {exact: true}).waitFor();
    await page.getByText("L16 audio active", {exact: true}).waitFor();

    // Navigate away from an active call: component teardown owns cancellation and cleanup.
    await page.setViewportSize({width: 390, height: 844});
    await page.reload();
    await page.getByRole("heading", {name: "See a voice session happen."}).waitFor();
    const labWidth = await page.getByTestId("protocol-lab").evaluate((element) => element.scrollWidth);
    assert.ok(labWidth <= 390, `mobile lab overflowed: ${labWidth}px`);
    assert.deepEqual(pageErrors, [], `browser page errors: ${pageErrors.join("; ")}`);
  } finally {
    await browser?.close();
    server.kill("SIGTERM");
    await new Promise((resolve) => {
      if (server.exitCode !== null) return resolve();
      server.once("exit", resolve);
      setTimeout(resolve, 2_000).unref();
    });
    if (server.exitCode && server.exitCode !== 143) {
      assert.fail(`docs server failed (${server.exitCode}):\n${serverOutput}`);
    }
  }
});
