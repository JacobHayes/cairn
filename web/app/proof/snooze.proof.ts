// The proof's media for brief 7.4 (briefs/proof/7.4/prove.sh): a screenshot of each state of
// snoozing a container from its detail, written to CAIRN_PROOF_OUT, and the next list's keys
// before and after as snooze.json. Each step asserts what its picture shows.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

import { daysAfter, nextItem, nextKeys, openActing } from "../e2e/acting.ts";
import { openNode, section } from "../e2e/detail.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
/** A screenshot with the receipt toast away, which would cover the corner of what is shown. */
async function shot(shown: Page | Locator, name: string): Promise<void> {
  const page = "page" in shown ? shown.page() : shown;
  const dismiss = page.getByRole("button", { name: "Dismiss" });
  if ((await dismiss.count()) > 0) {
    await dismiss.first().click();
  }
  await shown.screenshot({ path: join(out, `${name}.png`) });
}

test.use({ viewport: { width: 1200, height: 800 } });

test("a container snoozed from its detail, and unsnoozed from a descendant's", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "next");
  const before = await nextKeys(page);
  const today = (await page.getByTestId("derivation").getAttribute("data-today")) ?? "";
  const until = daysAfter(today, 14);
  const group = await openNode(page, "browser", "j_launch", "n_materials");
  const blocking = await section(group, "blocking");
  await blocking.getByRole("button", { name: "Snooze until a date" }).click();
  await blocking.getByLabel("Snooze until").fill(until);
  await blocking.getByTestId("snooze").getByRole("button", { name: "Save" }).click();
  await expect(blocking.getByTestId("snooze")).toContainText(`Snoozed until ${until}`);
  await shot(blocking, "1-snoozing-a-group-from-its-detail");

  await page.getByTestId("nav-next").click();
  await expect(nextItem(page, "n_beta_end")).toBeVisible();
  await expect(nextItem(page, "n_docs")).toHaveCount(0);
  const during = await nextKeys(page);
  await shot(page, "2-the-next-list-without-the-branch");

  const part = await openNode(page, "browser", "j_launch", "n_docs");
  const held = await section(part, "blocking");
  const through = held.getByTestId("snoozed-via");
  await expect(through).toContainText("Snoozed through");
  await shot(held, "3-a-descendant-names-its-container");

  await through.getByRole("button", { name: "Unsnooze Launch materials" }).click();
  await page.getByTestId("nav-next").click();
  await expect(nextItem(page, "n_docs")).toBeVisible();
  const after = await nextKeys(page);
  writeFileSync(join(out, "snooze.json"), JSON.stringify({ before, during, after }, null, 2));
});
