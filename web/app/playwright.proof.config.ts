// The proof's media run (briefs/proof/<brief>/prove.sh): the app's tests' servers and browser,
// running one proof/*.proof.ts, which keeps a screenshot of each acceptance state and a video
// of the main flow in CAIRN_PROOF_OUT. Not a browser-step suite: it navigates and captures, and waits
// for the state it pictures; the e2e specs assert.
import { defineConfig } from "@playwright/test";

import base from "./playwright.config.ts";

export default defineConfig({
  ...base,
  testDir: "proof",
  testMatch: "**/*.proof.ts",
  outputDir: "dist/proof-results",
  use: { ...base.use, viewport: { width: 1100, height: 720 } },
});
