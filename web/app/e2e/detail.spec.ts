// Brief 8.7, Acceptance: the inspector's one write path that no unit test reaches, over the
// in-browser host (each page load seeds the fixtures afresh): a date pin the chain rejects
// (F5), with the rejection and an unsent bypass reason surviving a reload (D4), counted by the
// sync chip wherever you go, and resolved with a listed move. The inspector's sections,
// forms and offers are vitest's.
import { expect, test } from "@playwright/test";

import { openNode, pin } from "./detail.ts";
import { fresh, goWithin, menuItem, syncChip } from "./shell.ts";

test("a later pin is rejected with its chain, kept across a reload and counted away from it, and resolved with a listed move (F5)", async ({ page }) => {
  let panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  let conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await expect(conflict.getByTestId("chain")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-state", "not-saved");
  await expect(panel.getByTestId("pin-date")).toHaveText("Nov 2");
  // The rejection and a reason typed meanwhile are kept per tab, so a reload loses neither.
  await menuItem(panel, "done-anyway");
  const reason = fresh("Reviewed out of band");
  await panel.getByTestId("bypass").getByLabel("Why bypass the guard").fill(reason);
  await page.reload();
  panel = page.getByTestId("node-detail");
  conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await expect(panel.getByTestId("bypass").getByLabel("Why bypass the guard")).toHaveValue(reason);
  // Away from the control it is still counted, and Go to it comes back to it.
  await goWithin(page, "/library?type=routes");
  await expect(syncChip(page)).toHaveText("NOT SAVED · 1");
  await syncChip(page).click();
  const needsYou = page.getByTestId("sync-popover").getByTestId("sync-problem");
  await expect(needsYou).toContainText("Final report");
  await needsYou.getByRole("button", { name: "Go to it" }).click();
  await expect(conflict).toBeVisible();
  await conflict.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(conflict).toHaveCount(0);
  await expect(panel.getByTestId("pin-date")).toHaveText("Nov 20");
  await expect(panel.getByTestId("receipt").first()).toHaveText(/Saved/);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});
