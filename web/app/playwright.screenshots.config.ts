// `mise run screenshots`: the app's browser setup (playwright.config.ts) running only
// e2e/screenshots.run.ts, on the in-browser host's built demo site (no binary), which writes the
// pictures to CAIRN_SCREENSHOTS_DIR. Not a browser-step suite: it navigates and captures, and
// waits for each state; the e2e specs assert.
import { defineConfig, devices } from "@playwright/test";

import base from "./playwright.config.ts";

process.env["CAIRN_E2E_BINARY"] = "0";

export default defineConfig({
  ...base,
  testMatch: "screenshots.run.ts",
  timeout: 900_000,
  outputDir: "dist/screenshots-results",
  globalTeardown: "./e2e/screenshots.teardown.ts",
  projects: [{ name: "screenshots", fullyParallel: true, use: { ...devices["Desktop Chrome"], actionTimeout: 15_000 } }],
});
