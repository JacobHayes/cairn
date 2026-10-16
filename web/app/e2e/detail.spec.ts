// Brief 8.7, Acceptance: the inspector's one write path that no unit test reaches, over the
// in-browser host (each page load seeds the fixtures afresh): a date pin the chain rejects
// (F5), with the rejection and an unsent bypass reason surviving a reload (D4), counted by the
// sync chip wherever you go, and resolved with a listed move. A node's actions hold one open
// form, which a reload brings back (D1, B6), also from the drafts an older build kept. The
// inspector's sections, forms and offers are vitest's.
import { expect, test } from "@playwright/test";

import { openNode, pin } from "./detail.ts";
import { fresh, goWithin, menuItem, nodeCard, nodePanel, syncChip } from "./shell.ts";

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
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
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

test("a node's actions hold one open form, and a reload brings back that one (D1, B6)", async ({ page }) => {
  let panel = await openNode(page, "browser", "j_launch", "n_announcement");
  await menuItem(panel, "skip");
  const reason = fresh("Waiting on legal");
  await panel.getByTestId("skip-form").getByLabel("Why skip it").fill(reason);
  // Another choice, from the graph's own menu, replaces the form: two are never open.
  await nodeCard(page, "n_announcement").click({ button: "right" });
  await page.getByTestId("context-snooze").click();
  await expect(panel.getByTestId("snooze")).toBeVisible();
  await expect(panel.getByTestId("skip-form")).toHaveCount(0);
  await panel.getByRole("radio", { name: "A date" }).check();
  await panel.getByLabel("Snooze until", { exact: true }).fill("2026-11-02");
  await page.reload();
  panel = nodePanel(page, "n_announcement");
  await expect(panel.getByTestId("snooze")).toBeVisible();
  await expect(panel.getByTestId("skip-form")).toHaveCount(0);
  await expect(panel.getByRole("radio", { name: "A date" })).toBeChecked();
  await expect(panel.getByLabel("Snooze until", { exact: true })).toHaveValue("2026-11-02");
  // Cancel closes it, and the row is back.
  await panel.getByTestId("snooze").getByRole("button", { name: "Cancel" }).click();
  await expect(panel.getByTestId("snooze")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Mark done" })).toBeVisible();
});

test("choosing Mark done again from the graph's menu keeps what was typed in its form (D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_launch", "n_docs");
  await panel.getByRole("button", { name: "Done..." }).click();
  const typed = fresh("https://example.test/notes");
  await panel.getByLabel("Artifact address").fill(typed);
  await nodeCard(page, "n_docs").click({ button: "right" });
  await page.getByTestId("context-done-with-evidence").click();
  await expect(panel.getByLabel("Artifact address")).toHaveValue(typed);
});

test("drafts an older build kept for several forms open one, the first the old screen showed (D1)", async ({ page }) => {
  // The older build kept a draft per form; a tab open across the deploy still holds them.
  await page.addInitScript(() => {
    if (sessionStorage.getItem("seeded") === null) {
      sessionStorage.setItem("seeded", "yes");
      const seen = { base: 1, deployment: 1 };
      sessionStorage.setItem("cairn:draft:skip:j_launch:n_announcement", JSON.stringify({ value: "Typed under the old build", ...seen }));
      sessionStorage.setItem("cairn:draft:snooze:j_launch:n_announcement", JSON.stringify({ value: { aside: "n_announcement", until: "tomorrow", date: "2026-10-07", target: "" }, ...seen }));
    }
  });
  const panel = await openNode(page, "browser", "j_launch", "n_announcement");
  await expect(panel.getByTestId("skip-form").getByLabel("Why skip it")).toHaveValue("Typed under the old build");
  await expect(panel.getByTestId("snooze")).toHaveCount(0);
  const kept = await page.evaluate(() => Object.keys(sessionStorage).filter((key) => key.startsWith("cairn:draft:")));
  expect(kept).toEqual(["cairn:draft:actions:j_launch:n_announcement"]);
});

test("Discard on the chip clears a section's rejection on its screen, and it does not come back after a reload", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  await expect(panel.getByTestId("date-conflict")).toBeVisible();
  await syncChip(page).click();
  await page.getByTestId("sync-popover").getByTestId("sync-problem").getByRole("button", { name: "Discard" }).click();
  await expect(panel.getByTestId("date-conflict")).toHaveCount(0);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
  await page.reload();
  await expect(page.getByTestId("node-detail")).toBeVisible();
  await expect(page.getByTestId("date-conflict")).toHaveCount(0);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});
