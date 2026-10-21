// Brief 8.7, Acceptance: the inspector's one write path that no unit test reaches, over the
// in-browser host (each page load seeds the fixtures afresh): a date pin the chain rejects
// (F5), with the rejection and an unsent bypass reason surviving a reload (D4), counted by the
// sync chip wherever you go, and resolved with a listed move. A node's actions hold one open
// form, which a reload brings back (D1, B6). The inspector's sections, forms and offers are
// vitest's.
import type { Page } from "@playwright/test";

import { checkpoint, expect, test } from "./test.ts";

import { openNode, pin, section } from "./detail.ts";
import { fresh, goWithin, menuItem, nodeCard, nodePanel, openAt, openFromCanvas, openSheet, syncChip } from "./shell.ts";

async function discardClearsRejection(page: Page) {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  // Discard on the chip clears a section's rejection on its screen, and it does not come back after a reload.
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
  await checkpoint(page);
}

async function rejectionSurvivesAndResolves(page: Page) {
  let panel = page.getByTestId("node-detail");
  await pin(panel, "2026-11-25");
  let conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await conflict.getByText("Why it is short").click();
  await expect(conflict.getByTestId("chain")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-state", "not-saved");
  await expect(panel.locator('[data-testid="date"][data-label="Due"]')).toContainText("Nov 2");
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
  await goWithin(page, "/routes?type=routes");
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await syncChip(page).click();
  const needsYou = page.getByTestId("sync-popover").getByTestId("sync-problem");
  await expect(needsYou).toContainText("Final report");
  await needsYou.getByRole("button", { name: "Go to it" }).click();
  await expect(conflict).toBeVisible();
  await conflict.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(conflict).toHaveCount(0);
  await expect(panel.locator('[data-testid="date"][data-label="Due"]')).toContainText("Nov 20");
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
  await checkpoint(page);
}

async function oneOpenFormSurvivesReload(page: Page) {
  let panel = await openNode(page, "browser", "j_launch", "n_announcement");
  await menuItem(panel, "skip");
  const waiting = fresh("Waiting on legal");
  await panel.getByTestId("skip-form").getByLabel("Why skip it").fill(waiting);
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
  await checkpoint(page);
}

async function historyNamesWhoDidWhat(page: Page) {
  // Details is last and folded, and its history says who did what: others by name, the viewer as You, never an id.
  const panel = nodePanel(page, "n_announcement");
  const details = panel.getByTestId("details");
  await expect(details).not.toHaveAttribute("open", "");
  expect(await panel.locator(".detail-folds > details").last().getAttribute("data-testid")).toBe("details");
  const history = (await section(panel, "details")).getByTestId("history-patch");
  await expect(history.first().getByTestId("history-sentence")).not.toBeEmpty();
  await expect(history.first()).toHaveAttribute("data-actor", /^u_/);
  await expect(history.first().getByTestId("history-sentence")).not.toContainText("u_");
  const before = await history.count();
  await panel.getByRole("button", { name: "Start" }).click();
  await expect(history).toHaveCount(before + 1);
  await expect(history.last().getByTestId("history-sentence")).toHaveText(/^You /);
  await checkpoint(page);
}

async function markDoneKeepsTypedText(page: Page) {
  // Choosing Mark done again from the graph's menu keeps what was typed in its form.
  const docs = await openFromCanvas(page, "n_docs");
  await docs.getByRole("button", { name: "Mark done…" }).click();
  const typed = fresh("https://example.test/notes");
  await docs.getByLabel("Artifact address").fill(typed);
  await nodeCard(page, "n_docs").click({ button: "right" });
  await page.getByTestId("context-done-with-evidence").click();
  await expect(docs.getByLabel("Artifact address")).toHaveValue(typed);
}

test("a later pin is rejected with its chain, kept across a reload and counted away from it, discarded or resolved with a listed move (F5); a node's actions hold one open form; its history says who did what", async ({ page }) => {
  await discardClearsRejection(page);
  await rejectionSurvivesAndResolves(page);
  await oneOpenFormSurvivesReload(page);
  await historyNamesWhoDidWhat(page);
  await markDoneKeepsTypedText(page);
});

test("a node held by its container's snooze offers Unsnooze of the container, which lifts it (B6)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_launch", "n_announcement");
  await menuItem(panel, "snooze");
  await panel.getByRole("radio", { name: /^All of / }).first().check();
  await panel.getByTestId("snooze").getByRole("button", { name: "Snooze", exact: true }).click();
  const unsnooze = panel.getByTestId("actions").getByRole("button", { name: /^Unsnooze / });
  await expect(unsnooze).toBeVisible();
  await unsnooze.click();
  await expect(unsnooze).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Mark done" })).toBeVisible();
});

test("a milestone done on a phone: an empty date says what to choose and blocks Mark done, done leaves its sentence a readable line, and editing the date offers Save only for a new one", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openAt(page, "browser", "j_vendor_eval", "list", { node: "n_decision_meeting" });
  await openSheet(page);
  const panel = page.getByTestId("node-detail");
  await panel.getByLabel("Done on").fill("");
  await expect(panel.getByText("Choose the date it was done.")).toBeVisible();
  await expect(panel.getByRole("button", { name: "Mark done", exact: true })).toBeDisabled();
  await panel.getByLabel("Done on").fill("2026-10-20");
  await expect(panel.getByText("Choose the date it was done.")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Mark done", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Mark done", exact: true }).click();
  await expect(panel.getByTestId("receipt")).toBeVisible();
  const lead = (await panel.locator(".detail-lead").boundingBox())?.width ?? 0;
  const sentence = (await panel.locator(".detail-lead > :first-child").boundingBox())?.width ?? 0;
  expect(sentence).toBeGreaterThan(lead / 2);
  await checkpoint(page);
  await menuItem(panel, "edit-date");
  const form = panel.getByTestId("edit-date-form");
  await expect(form.getByRole("button", { name: "Save" })).toBeDisabled();
  await form.getByLabel("Done on").fill("2026-10-19");
  await expect(form.getByRole("button", { name: "Save" })).toBeEnabled();
});
