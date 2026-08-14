import assert from "node:assert/strict";
import {access} from "node:fs/promises";
import {spawn} from "node:child_process";
import test from "node:test";

import {chromium} from "playwright-core";

const port = 31846;
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
      const response = await fetch(`${origin}/rtvbp/docs/intro`);
      if (response.ok) return;
    } catch {
      // Server is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("timed out waiting for the docs server");
}

test("documentation is a polished, responsive integration workspace", {timeout: 60_000}, async () => {
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
      args: ["--no-sandbox"],
    });
    const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
    page.setDefaultTimeout(5_000);
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));

    const response = await page.goto(`${origin}/rtvbp/docs/intro`);
    assert.equal(response?.ok(), true);
    await page.getByTestId("docs-gateway").waitFor({timeout: 2_000});
    await page.getByRole("heading", {name: "Build from your boundary."}).waitFor();
    await page.getByRole("link", {name: "Try it out", exact: true}).first().waitFor();

    const sidebar = page.locator(".theme-doc-sidebar-container");
    for (const label of ["Start here", "Build", "Understand", "Reference"]) {
      await sidebar.getByText(label, {exact: true}).first().waitFor();
    }

    await page.getByTestId("docs-gateway").locator('a[href$="/docs/getting-started/typescript"]').click();
    await page.waitForURL("**/docs/getting-started/typescript");
    await page.getByRole("heading", {name: "TypeScript and browser SDK"}).waitFor();
    await page.locator("article a").first().focus();
    const codeOutline = await page.locator("article a").first().evaluate((element) =>
      getComputedStyle(element).outlineStyle,
    );
    assert.notEqual(codeOutline, "none", "focused documentation links must remain visible");

    await page.goto(`${origin}/rtvbp/docs/reference/babelforce.v1/operations/session.initialize`);
    await page.getByRole("heading", {name: "session.initialize"}).waitFor();
    await page.getByText("voice → application", {exact: true}).waitFor();
    const referenceWidth = await page.locator("article").evaluate((element) => element.scrollWidth);
    const referenceClientWidth = await page.locator("article").evaluate((element) => element.clientWidth);
    assert.ok(referenceWidth <= referenceClientWidth + 1, "reference article must contain wide tables");

    const themeButton = page.getByRole("button", {name: /switch between dark and light mode/i});
    for (let attempt = 0; attempt < 3; attempt += 1) {
      if (await page.locator("html").getAttribute("data-theme") === "dark") break;
      await themeButton.click();
    }
    assert.equal(await page.locator("html").getAttribute("data-theme"), "dark");
    const darkBackground = await page.locator("body").evaluate((element) => getComputedStyle(element).backgroundColor);
    assert.notEqual(darkBackground, "rgb(255, 255, 255)");

    await page.setViewportSize({width: 390, height: 844});
    await page.goto(`${origin}/rtvbp/docs/getting-started/typescript`);
    const bodyWidth = await page.locator("body").evaluate((element) => ({
      client: element.clientWidth,
      scroll: element.scrollWidth,
    }));
    assert.ok(bodyWidth.scroll <= bodyWidth.client + 1, `mobile docs overflowed: ${bodyWidth.scroll}px`);
    await page.getByRole("button", {name: /toggle navigation bar/i}).click();
    await page.locator(".navbar-sidebar").getByText("Build", {exact: true}).last().waitFor();

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
