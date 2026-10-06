// The proof run (briefs/proof/4.6/prove.sh): the app's tests' servers and browser, running
// proof/shell.proof.ts, which keeps a screenshot of each acceptance state and a video of the
// main flow in CAIRN_PROOF_OUT. Not a rung 6 suite: it asserts what it shows, but its job is
// the pictures.
import { defineConfig } from "@playwright/test";

import base from "./playwright.config.ts";

export default defineConfig({
  ...base,
  testDir: "proof",
  testMatch: "**/*.proof.ts",
  outputDir: "dist/proof-results",
  use: { ...base.use, viewport: { width: 1100, height: 720 } },
});
