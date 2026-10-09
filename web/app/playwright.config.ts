// Rung 6's end-to-end tests of the app shell (PRACTICES, The validation ladder): Chromium
// against the app served by two Vite dev servers, one on the server host proxying the API to
// the fixture server (crates/wasm/examples/fixture_server.rs) and one on the in-browser host
// with no server (CAIRN_DEMO_PORT; e2e/shell.ts opens each host on its own); and the server
// host's suite again against the real binary, serving its embedded build and the API on one
// port over a database seeded with the fixtures (scripts/e2e-binary; brief 4.7). Each run
// serves on free ports, so runs beside one another do not collide; CAIRN_E2E_PORT names the
// binary's instead. `mise run
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
process.env["CAIRN_DEMO_PORT"] ??= String(await freePort());
const appPort = process.env["CAIRN_APP_PORT"];
const binaryPort = process.env["CAIRN_E2E_PORT"];
const repo = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: "e2e",
  outputDir: "dist/test-results",
  timeout: 120_000,
  expect: { timeout: 15_000 },
  forbidOnly: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${appPort}` },
  // The projects run side by side. The tests tagged @server are the ones only the server host
  // can run: pages meeting through the server (live updates, conflicts), sign-in, and the
  // assistant. They use the one fixture server, and the `binary` project the one binary, so
  // each of those runs its tests one at a time, in file order; they come first so they start
  // first. server.spec.ts runs only against the binary: it is the server host's shell suite,
  // and the binary serves the same API the fixture server does. A test on the in-browser host
  // seeds its own store in its page, so those run in parallel in the workers left over.
  projects: [
    { name: "server", grep: /@server/, testIgnore: "server.spec.ts", workers: 1, use: { ...devices["Desktop Chrome"] } },
    {
      name: "binary",
      testMatch: "server.spec.ts",
      workers: 1,
      use: { ...devices["Desktop Chrome"], baseURL: `http://127.0.0.1:${binaryPort}` },
    },
    { name: "chromium", grepInvert: /@server/, fullyParallel: true, use: { ...devices["Desktop Chrome"] } },
  ],
  // One entry, so Playwright does not start the four servers one after another:
  // scripts/e2e-servers starts them side by side (the fixture server, the real binary, and the
  // two Vite dev servers) and prints READY when all of them answer.
  webServer: {
    command: "scripts/e2e-servers",
    cwd: repo,
    wait: { stdout: /READY/ },
    timeout: 600_000,
    reuseExistingServer: false,
    // scripts/e2e-servers writes the servers' logs (a JSON line per request) to a file.
    stdout: "ignore",
  },
});
