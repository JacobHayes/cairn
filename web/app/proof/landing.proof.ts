// The proof's picture for the landing (briefs/proof/2026-10-16-landing-without-a-jump/prove.sh):
// the screen a person sees on opening `/`, written to CAIRN_PROOF_OUT.
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { visit } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";

test("opening Cairn shows the journey index", async ({ page }) => {
  await visit(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await page.screenshot({ path: join(out, "1-opening-cairn.png") });
});
