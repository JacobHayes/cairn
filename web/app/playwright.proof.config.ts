// A change's screenshots and video run: the app's tests' browser and `cairn demo` (a fresh one for
// each test, as the browser tests have), running the *.proof.ts specs in CAIRN_PROOF_DIR. Proof specs are not kept in the
// repository (AGENTS, Show a change people can see): one lives in its change's scratch directory
// and saves a screenshot of each state and a video of the main flow in that directory. Not
// a browser-step suite: it navigates and captures, and waits for the state it pictures; the
// e2e specs assert.
//
// tsconfig.proof.json lets such a spec import `@playwright/test`.
//
// usage: mise run proof <dir>, which builds the web app and then runs
// (cd web/app && CAIRN_PROOF_DIR=<dir> npx playwright test -c playwright.proof.config.ts)
import { resolve } from "node:path";

import { defineConfig } from "@playwright/test";

import base from "./playwright.config.ts";

const proofDir = process.env["CAIRN_PROOF_DIR"];
if (proofDir === undefined || proofDir === "") {
  throw new Error("set CAIRN_PROOF_DIR to the directory holding the proof's *.proof.ts");
}

export default defineConfig({
  ...base,
  testDir: resolve(proofDir),
  testMatch: "**/*.proof.ts",
  outputDir: "dist/proof-results",
  tsconfig: "tsconfig.proof.json",
});
