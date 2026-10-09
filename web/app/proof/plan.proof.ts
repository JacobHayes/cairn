// The proof's pictures for the Plan list and timeline (briefs/proof/8.10): the tree, a sorted
// (flat) list, the decision columns, a selection, and the timeline with a selected row's float
// and without a final milestone, written to CAIRN_PROOF_OUT. Navigates and captures; the e2e
// specs assert. In-browser host, at the fixed day the browser tests read the fixtures on.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { openActing } from "../e2e/acting.ts";
import { openAt } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1280, height: 760 } });

test("the list as a tree, flat when sorted, as decisions, and with a selection", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list");
  await page.getByTestId("list-fold").first().click();
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  await shot(page, "1-tree");
  await page.getByRole("button", { name: "Sort by due" }).click();
  await expect(page.getByTestId("list-fold")).toHaveCount(0);
  await shot(page, "2-sorted-flat");
  await page.setViewportSize({ width: 1500, height: 760 });
  await openActing(page, "browser", "j_vendor_eval", "plan/list?decisions=1");
  await expect(page.locator('[data-testid="list-row"][data-node="n_partner_runs"]')).toBeVisible();
  await shot(page, "3-decisions");
  await page.setViewportSize({ width: 1280, height: 760 });
  await openActing(page, "browser", "j_launch", "plan/list?sort=due");
  await expect(page.getByTestId("list-row").nth(1)).toBeVisible();
  await page.getByTestId("list-row").nth(0).locator("input[type=checkbox]").click();
  await page.getByTestId("list-row").nth(1).locator("input[type=checkbox]").click();
  await expect(page.getByTestId("bulk-bar")).toBeVisible();
  await shot(page, "4-selection");
});

test("the timeline with a selected row's float, and without a final milestone", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/timeline?detail=all", { node: "n_final_report", fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("timeline-float")).toHaveCount(1);
  await shot(page, "5-timeline-selected");
  await openAt(page, "browser", "j_hiring", "plan/timeline", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("timeline-no-end")).toBeVisible();
  await shot(page, "6-timeline-no-end");
});
