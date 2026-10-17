// The proof's media run (briefs/proof/<brief>/prove.sh): the app's tests' browser and the Vite dev server (the binary only when asked),
// running one proof/*.proof.ts, which keeps a screenshot of each acceptance state and a video
// of the main flow in CAIRN_PROOF_OUT. Not a browser-step suite: it navigates and captures, and waits
// for the state it pictures; the e2e specs assert.
import { defineConfig } from "@playwright/test";

import base from "./playwright.config.ts";

// A picture needs the in-browser host on the Vite dev server, which builds and starts nothing
// else. The binary (`cairn demo`, with the web build it embeds) starts only for a proof of the
// server host, whose prove.sh sets CAIRN_E2E_BINARY=1.
process.env["CAIRN_E2E_BINARY"] ??= "0";

export default defineConfig({
  ...base,
  testDir: "proof",
  testMatch: "**/*.proof.ts",
  outputDir: "dist/proof-results",
  use: { ...base.use, viewport: { width: 1100, height: 720 } },
});
