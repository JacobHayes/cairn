// Rung 6's end-to-end tests of the app shell (PRACTICES, The validation ladder): Chromium
// against the app served by Vite, over the in-browser host (`?host=browser`) and over the
// server host (`?host=server`), Vite proxying the API to the fixture server
// (crates/wasm/examples/fixture_server.rs); and the server host's suite again against the
// real binary, serving its embedded build and the API on one port over a database seeded
// with the fixtures (scripts/e2e-binary; brief 4.7). Each run serves on free ports, so runs
// beside one another do not collide; CAIRN_E2E_PORT names the binary's instead. `mise run
// check:6` builds the module, the web build, the fixture server, and the binary first.
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";

import { defineConfig, devices } from "@playwright/test";

/** A port nothing listens on now. */
function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => {
        if (address === null || typeof address === "string") {
          reject(new Error("no port"));
        } else {
          resolve(address.port);
        }
      });
    });
  });
}

// Chosen once, in the runner; its workers inherit them through the environment.
process.env["CAIRN_APP_PORT"] ??= String(await freePort());
process.env["CAIRN_SERVER_PORT"] ??= String(await freePort());
process.env["CAIRN_E2E_PORT"] ??= String(await freePort());
const appPort = process.env["CAIRN_APP_PORT"];
const serverPort = process.env["CAIRN_SERVER_PORT"];
const binaryPort = process.env["CAIRN_E2E_PORT"];
const server = `http://127.0.0.1:${serverPort}`;
const repo = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: "e2e",
  outputDir: "dist/test-results",
  timeout: 120_000,
  expect: { timeout: 15_000 },
  forbidOnly: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${appPort}` },
  // The projects run side by side. The tests tagged @server use the one fixture server (they
  // write to it, or compare with what it serves), and the `binary` project the one binary, so
  // each of those runs its tests one at a time, in file order, as they ran before the projects
  // were split; they come first so they start first, being the longest chains. A test on the
  // in-browser host seeds its own store in its page, so those run in parallel in the workers
  // left over.
  projects: [
    { name: "server", grep: /@server/, workers: 1, use: { ...devices["Desktop Chrome"] } },
    {
      name: "binary",
      testMatch: "server.spec.ts",
      workers: 1,
      use: { ...devices["Desktop Chrome"], baseURL: `http://127.0.0.1:${binaryPort}` },
    },
    { name: "chromium", grepInvert: /@server/, fullyParallel: true, use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: [
    {
      command: `scripts/built examples/fixture_server ${serverPort} http://127.0.0.1:${appPort}`,
      cwd: repo,
      url: `${server}/capabilities`,
      timeout: 600_000,
      reuseExistingServer: false,
      stdout: "ignore",
    },
    {
      command: `scripts/e2e-binary ${binaryPort}`,
      cwd: repo,
      url: `http://127.0.0.1:${binaryPort}/capabilities`,
      timeout: 600_000,
      reuseExistingServer: false,
      stdout: "ignore",
    },
    {
      command: "node ../../node_modules/vite/bin/vite.js --config vite.config.ts",
      env: { CAIRN_SERVER: server, CAIRN_APP_PORT: appPort },
      url: `http://127.0.0.1:${appPort}/`,
      timeout: 120_000,
      reuseExistingServer: false,
      stdout: "ignore",
    },
  ],
});
