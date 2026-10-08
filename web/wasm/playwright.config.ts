// Rung 6's in-browser end-to-end tests of the browser host (PRACTICES, The validation
// ladder): Chromium against the wasm build, the page served by Vite (vite.config.ts).
// `mise run check:6` builds the module and the server's cases first.
import { execFileSync } from "node:child_process";

import { defineConfig, devices } from "@playwright/test";

/**
 * The port the page is served on: CAIRN_E2E_PORT when set, otherwise one the system has
 * free now, so runs in other workspaces at the same time never collide. It is chosen once,
 * in the runner, and passed on in the environment, which the test workers and the server
 * inherit and read back here.
 */
function servingPort(): string {
  const chosen = process.env["CAIRN_E2E_PORT"];
  if (chosen !== undefined && chosen !== "") {
    return chosen;
  }
  const probe =
    'const s = require("node:net").createServer(); s.listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });';
  const free = execFileSync(process.execPath, ["-e", probe], { encoding: "utf8" }).trim();
  process.env["CAIRN_E2E_PORT"] = free;
  return free;
}

const port = servingPort();

export default defineConfig({
  testDir: "e2e",
  outputDir: "dist/test-results",
  timeout: 120_000,
  // Every test loads the module into a page of its own, so they run in parallel.
  fullyParallel: true,
  forbidOnly: true,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${port}` },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "node ../../node_modules/vite/bin/vite.js --config vite.config.ts",
    url: `http://127.0.0.1:${port}/e2e/index.html`,
    env: { CAIRN_E2E_PORT: port },
    reuseExistingServer: false,
    stdout: "ignore",
  },
});
