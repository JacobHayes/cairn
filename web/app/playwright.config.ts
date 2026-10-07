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
  fullyParallel: false,
  workers: 1,
  forbidOnly: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${appPort}` },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    {
      name: "binary",
      testMatch: "server.spec.ts",
      use: { ...devices["Desktop Chrome"], baseURL: `http://127.0.0.1:${binaryPort}` },
    },
  ],
  webServer: [
    {
      command: `cargo run --quiet --locked -p cairn-wasm --example fixture_server -- ${serverPort}`,
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
