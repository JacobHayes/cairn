// Several cards selected on the graph, in light and dark, written to CAIRN_PROOF_OUT: the members
// tinted with no trace, and the inspector's list of them over the actions.
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { nodeCard, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";

test.use({ viewport: { width: 1300, height: 800 } });

for (const scheme of ["light", "dark"] as const) {
  test(`three selected, ${scheme}`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await openJourney(page, "browser", "j_vendor_eval");
    await nodeCard(page, "n_plan").getByTestId("card-open").click();
    await nodeCard(page, "n_testing").click({ modifiers: ["Shift"] });
    await nodeCard(page, "n_decision_meeting").click({ modifiers: ["Shift"] });
    await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "3");
    await page.getByTestId("selection-status").click();
    await page.screenshot({ path: join(out, `selection-${scheme}.png`) });
  });
}
