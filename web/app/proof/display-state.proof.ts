// The proof's pictures for display state (briefs/proof/8.1/prove.sh): the detail header of a
// node an answer ruled out, and the canvas chips of a journey mid-way, written to
// CAIRN_PROOF_OUT. Each step asserts what its picture shows. The in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { openNode, state } from "../e2e/detail.ts";
import { nodeCard, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";

test.use({ viewport: { width: 1280, height: 800 } });

test("a node an answer ruled out says so, and the canvas chips read the same words", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_partner_results");
  await expect(state(panel)).toHaveText("not relevant");
  await panel.getByTestId("detail-header").screenshot({ path: join(out, "1-ruled-out-header.png") });
  await page.goBack();
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(nodeCard(page, "n_partner_led").getByTestId("card-state")).toHaveText("not relevant");
  await expect(nodeCard(page, "n_final_review").getByTestId("card-state")).toHaveText("blocked");
  await page.screenshot({ path: join(out, "2-canvas-chips.png") });
});
