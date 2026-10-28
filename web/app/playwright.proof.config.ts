// A change's screenshots and video run: the app's tests' browser and the built demo site (the binary only when
// asked), running the *.proof.ts specs in CAIRN_PROOF_DIR. Proof specs are not kept in the
// repository (AGENTS, Show a change people can see): one lives in its change's scratch directory
// and saves a screenshot of each state and a video of the main flow in that directory. Not
// a browser-step suite: it navigates and captures, and waits for the state it pictures; the
// e2e specs assert.
//
// tsconfig.proof.json lets such a spec import `@playwright/test`.
//
// usage: mise run proof <dir>, which builds what the hosts need and then runs
// (cd web/app && CAIRN_PROOF_DIR=<dir> npx playwright test -c playwright.proof.config.ts)
import { resolve } from "node:path";

import { defineConfig } from "@playwright/test";

import base from "./playwright.config.ts";

const proofDir = process.env["CAIRN_PROOF_DIR"];
if (proofDir === undefined || proofDir === "") {
  throw new Error("set CAIRN_PROOF_DIR to the directory holding the proof's *.proof.ts");
}

// A picture needs the in-browser host on the built demo site, which starts no binary. The binary (`cairn demo`, with the web build it embeds) starts only for a proof of the
// server host, run with CAIRN_E2E_BINARY=1.
process.env["CAIRN_E2E_BINARY"] ??= "0";

export default defineConfig({
  ...base,
  testDir: resolve(proofDir),
  testMatch: "**/*.proof.ts",
  outputDir: "dist/proof-results",
  tsconfig: "tsconfig.proof.json",
});
