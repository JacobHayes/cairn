// The browser step's end-to-end tests of the app shell (PRACTICES, The validation ladder): Chromium
// against `cairn demo` (crates/cairn/src/demo.rs), the real binary over the fixtures in memory,
// serving its web build and the API on one port. Every test starts a demo of its own
// (e2e/test.ts), so tests run side by side on free ports and none sees what another wrote.
// `mise run check` builds the web build and the binary first.
import { availableParallelism } from "node:os";

import { defineConfig, devices } from "@playwright/test";

// One worker per core, at most six: each worker is a Chromium and a `cairn demo`, and more of them on a
// many-core machine crashed pages ("Target crashed") under load.
const WORKERS_MAX = 6;

export default defineConfig({
  testDir: "e2e",
  outputDir: "dist/test-results",
  timeout: 120_000,
  expect: { timeout: 15_000 },
  forbidOnly: true,
  reporter: [["list"]],
  fullyParallel: true,
  workers: Math.min(availableParallelism(), WORKERS_MAX),
  // A trace is kept only for a failure: a flake in a long run leaves what happened (no measurable cost on green runs).
  // Reduced motion turns the app's motion off (DESIGN, Motion), so nothing a test reads is mid-animation.
  use: { trace: "retain-on-failure", contextOptions: { reducedMotion: "reduce" } },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    // The structural walks (`mise run screenshots`, `mise run sweep`): not tests of behaviour but
    // rules run over every screen state, so they match their own files.
    { name: "screenshots", testMatch: "screenshots.run.ts", timeout: 900_000, use: { ...devices["Desktop Chrome"], actionTimeout: 15_000 } },
    { name: "sweep", testMatch: "breakpoints.run.ts", timeout: 900_000, use: { ...devices["Desktop Chrome"], actionTimeout: 15_000 } },
  ],
  globalSetup: "./e2e/demo.setup.ts",
  globalTeardown: "./e2e/screenshots.teardown.ts",
});
