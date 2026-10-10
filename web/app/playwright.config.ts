// Rung 6's end-to-end tests of the app shell (PRACTICES, The validation ladder): Chromium
// against the app on its two hosts. The server host is `cairn demo` (crates/cairn/src/demo.rs):
// the real binary over the fixtures in memory, serving its embedded web build and the API on
// one port. The in-browser host is the app's Vite dev server with no server behind it
// (CAIRN_DEMO_PORT; e2e/shell.ts opens each host on its own). Each run serves on free ports,
// so runs beside one another do not collide. `mise run check:6` builds the module, the web
// build, and the binary first.
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
process.env["CAIRN_SERVER_PORT"] ??= String(await freePort());
process.env["CAIRN_DEMO_PORT"] ??= String(await freePort());
const serverPort = process.env["CAIRN_SERVER_PORT"];
const repo = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: "e2e",
  outputDir: "dist/test-results",
  timeout: 120_000,
  expect: { timeout: 15_000 },
  forbidOnly: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${serverPort}` },
  // The projects run side by side. The tests tagged @server are the ones only the server host
  // can run: pages meeting through the server (live updates, conflicts), sign-in, and the
  // assistant. They share the one binary, whose scripted assistant is global, so they run one
  // at a time, in file order, and start first. A test on the in-browser host seeds its own
  // store in its page, so those run in parallel in the workers left over.
  projects: [
    { name: "server", grep: /@server/, workers: 1, use: { ...devices["Desktop Chrome"] } },
    { name: "chromium", grepInvert: /@server/, fullyParallel: true, use: { ...devices["Desktop Chrome"] } },
  ],
  // One entry, so Playwright does not start the two servers one after another:
  // scripts/e2e-servers starts them side by side (the binary and the Vite dev server) and
  // prints READY when both answer.
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
