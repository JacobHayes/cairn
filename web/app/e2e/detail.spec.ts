// Brief 8.7, Acceptance: the inspector's one write path that no unit test reaches, over the
// in-browser host (each page load seeds the fixtures afresh): a date pin the chain rejects
// (F5), with the rejection and an unsent bypass reason surviving a reload (D4), counted by the
// sync chip wherever you go, and resolved with a listed move. A node's actions hold one open
// form, which a reload brings back (D1, B6). The inspector's sections, forms and offers are
// vitest's.
import type { Locator, Page } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { frontierItem, openActing } from "./acting.ts";
import { openEditing, openForm } from "./authoring.ts";
import { openNode, pin, section, state } from "./detail.ts";
import { fresh, goTo, goWithin, menuItem, nodeCard, nodePanel, openAt, openFromCanvas, openJourney, openSheet, settled, syncChip, whenSettled } from "./shell.ts";

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
  await menuItem(panel, "skip");
  const reason = fresh("Covered elsewhere");
  await panel.getByTestId("skip-form").getByLabel("Why skip it").fill(reason);
  await page.reload();
  panel = page.getByTestId("node-detail");
  conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await expect(panel.getByTestId("skip-form").getByLabel("Why skip it")).toHaveValue(reason);
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
  expect(await whenSettled(page, () => panel.locator(".detail-folds > details").last().getAttribute("data-testid"))).toBe("details");
  const history = (await section(panel, "details")).getByTestId("history-patch");
  await expect(history.first().getByTestId("history-sentence")).not.toBeEmpty();
  await expect(history.first()).toHaveAttribute("data-actor", /^u_/);
  await expect(history.first().getByTestId("history-sentence")).not.toContainText("u_");
  const before = await whenSettled(page, () => history.count());
  // Its own date is ahead, and starting it is a plain move all the same.
  await panel.getByRole("button", { name: "Start", exact: true }).click();
  await expect(history).toHaveCount(before + 1);
  await expect(history.last().getByTestId("history-sentence")).toHaveText(/^You /);
  await checkpoint(page);
}

async function markDoneKeepsTypedText(page: Page) {
  // Choosing Mark done again from the graph's menu keeps what was typed in its form.
  const docs = await openFromCanvas(page, "n_docs");
  await docs.getByRole("button", { name: "Mark done…" }).click();
  const typed = fresh("https://example.test/notes");
  await docs.getByLabel("Artifact link").fill(typed);
  await nodeCard(page, "n_docs").click({ button: "right" });
  await page.getByTestId("context-done-with-evidence").click();
  await expect(docs.getByLabel("Artifact link")).toHaveValue(typed);
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
  const [leadBox, sentenceBox] = await whenSettled(page, () => Promise.all([panel.locator(".detail-lead").boundingBox(), panel.locator(".detail-lead > :first-child").boundingBox()]));
  const [lead, sentence] = [leadBox?.width ?? 0, sentenceBox?.width ?? 0];
  expect(sentence).toBeGreaterThan(lead / 2);
  await checkpoint(page);
  await menuItem(panel, "edit-date");
  const form = panel.getByTestId("edit-date-form");
  await expect(form.getByRole("button", { name: "Save" })).toBeDisabled();
  await form.getByLabel("Done on").fill("2026-10-19");
  await expect(form.getByRole("button", { name: "Save" })).toBeEnabled();
});

test("a stage is marked done with a reason: what was left inside reads skipped and what waited for it is freed, and Reopen takes it back (D1a)", async ({ page }) => {
  const stage = await openNode(page, "browser", "j_launch", "n_materials");
  await expect(nodeCard(page, "n_announcement").getByTestId("card-state")).not.toHaveAttribute("data-state", "skipped");
  await menuItem(stage, "mark-done");
  const form = stage.getByTestId("mark-done-form");
  await expect(form.getByRole("button", { name: "Mark done", exact: true })).toBeDisabled();
  await form.getByLabel("Why mark it done").fill(fresh("Covered by the website"));
  await form.getByRole("button", { name: "Mark done", exact: true }).click();
  await expect(stage.getByTestId("detail-state")).toHaveAttribute("data-status", "done");
  await expect(nodeCard(page, "n_announcement").getByTestId("card-state")).toHaveAttribute("data-state", "skipped");
  await expect(nodeCard(page, "n_docs").getByTestId("card-state")).toHaveAttribute("data-state", "skipped");
  await expect(nodeCard(page, "n_materials").getByTestId("card-marked-done")).toContainText("skipped");
  await menuItem(stage, "reopen");
  await expect(stage.getByTestId("detail-state")).not.toHaveAttribute("data-status", "done");
  await expect(nodeCard(page, "n_announcement").getByTestId("card-state")).not.toHaveAttribute("data-state", "skipped");
  await checkpoint(page);
});

/** A press as a hand makes it: the pointer goes down and up at one spot, so a control that moves away on the press (focus leaving a field, a row added) is missed. */
async function press(page: Page, control: Locator): Promise<void> {
  await control.scrollIntoViewIfNeeded();
  const box = await whenSettled(page, () => control.boundingBox());
  if (box === null) {
    throw new Error("nothing to press");
  }
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.up();
}

test("a checklist is written in the node form, ticked in place, finished with Mark done... from the list in one patch, and its progress is on the card (G5)", async ({ page }) => {
  // Author: require the checklist, add three items, move one, and type a fourth that Save must not drop.
  await openEditing(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_a");
  const completion = await openForm(page, "completion");
  await completion.getByLabel("Requires all items ticked before done").check();
  const add = completion.getByLabel("New item", { exact: true });
  for (const title of ["Rollback plan written", "Load test passed"]) {
    await add.fill(title);
    await add.press("Enter");
  }
  await add.fill("Support team briefed");
  await completion.getByRole("button", { name: "Add the item" }).click();
  await completion.getByRole("button", { name: "Move Support team briefed up" }).click();
  await add.fill("Status page drafted");
  // One press of Save, with the typed item still in the field, saves it too.
  await checkpoint(page);
  await press(page, page.getByTestId("node-form-save"));
  await expect(page.getByTestId("node-form-save")).toHaveCount(0);
  await settled(page);
  // Act: the checklist is in the inspector in the order written; ticking two moves the card's count.
  await openJourney(page, "browser", "j_bakeoff");
  const panel = await openFromCanvas(page, "n_trial_a");
  const checklist = panel.getByTestId("checklist");
  await expect(checklist.getByTestId("checklist-item")).toHaveText(["Rollback plan written", "Support team briefed", "Load test passed", "Status page drafted"]);
  await checkpoint(page);
  await checklist.getByTestId("checklist-item").nth(0).click();
  await checklist.getByTestId("checklist-item").nth(2).click();
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-checklist")).toHaveAttribute("data-ticked", "2");
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-checklist")).toHaveAttribute("data-total", "4");
  await checkpoint(page);
  // Finish from the list: the row opens the inspector's form, which ticks the rest and completes in one patch.
  await openActing(page, "browser", "j_bakeoff", "list?frontier=1");
  await frontierItem(page, "n_trial_a").getByRole("button", { name: "Mark done…" }).click();
  const done = nodePanel(page, "n_trial_a");
  await expect(done.getByTestId("done-ticks")).toBeVisible();
  await expect(done.getByTestId("done-evidence").locator("li")).toHaveCount(2);
  await checkpoint(page);
  await done.getByTestId("done-evidence").getByRole("button", { name: "Tick 2 items and mark done" }).click();
  await expect(done.getByTestId("detail-state")).toHaveText("done");
  await goTo(page, "graph");
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-checklist")).toHaveAttribute("data-ticked", "4");
  const history = (await section(done, "details")).getByTestId("history-patch");
  await expect(history.last().getByTestId("history-sentence")).toContainText("2 checklist items");
  await checkpoint(page);
  // Unticking one of finished work makes it stale, said by the item's title.
  const gone = done.getByTestId("checklist").getByTestId("checklist-item").nth(1);
  await gone.click();
  await expect(done.getByTestId("detail-sentence")).toContainText("Support team briefed");
});

test("leaving the new-item field adds the typed item and clears the field (G5)", async ({ page }) => {
  await openEditing(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_b");
  const completion = await openForm(page, "completion");
  await completion.getByLabel("New item", { exact: true }).fill("Left behind");
  await completion.getByLabel("Requires all items ticked before done").focus();
  await expect(completion.getByTestId("checklist-item-edit")).toHaveCount(1);
  await expect(completion.getByLabel("New item", { exact: true })).toHaveValue("");
});

test("one press of Save keeps an item typed in the new-item field when it is the only change (G5)", async ({ page }) => {
  await openEditing(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_b");
  const completion = await openForm(page, "completion");
  await completion.getByLabel("New item", { exact: true }).fill("Solo item");
  await expect(page.getByTestId("node-form-save")).toBeEnabled();
  await press(page, page.getByTestId("node-form-save"));
  await expect(page.getByTestId("node-form-save")).toHaveCount(0);
  await settled(page);
  await openJourney(page, "browser", "j_bakeoff");
  const panel = await openFromCanvas(page, "n_trial_b");
  await expect(panel.getByTestId("checklist").getByTestId("checklist-item")).toHaveText(["Solo item"]);
});

test("a checklist item typed and not added survives a reload, and a replacement for a removed ticked item starts unticked (G5)", async ({ page }) => {
  await openEditing(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_a");
  let completion = await openForm(page, "completion");
  await completion.getByLabel("Requires all items ticked before done").check();
  const add = completion.getByLabel("New item", { exact: true });
  await add.fill("Alpha");
  await add.press("Enter");
  await add.fill("Typed and not added");
  await page.reload();
  completion = await openForm(page, "completion");
  await expect(completion.getByLabel("New item", { exact: true })).toHaveValue("Typed and not added");
  await completion.getByLabel("New item", { exact: true }).fill("");
  await page.getByTestId("node-form-save").click();
  await settled(page);
  await openJourney(page, "browser", "j_bakeoff");
  const panel = await openFromCanvas(page, "n_trial_a");
  await panel.getByTestId("checklist").getByTestId("checklist-item").first().click();
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-checklist")).toHaveAttribute("data-ticked", "1");
  // Remove the ticked item and add one with the same title: it is a new item, so it carries no tick.
  await openEditing(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_a");
  completion = await openForm(page, "completion");
  await completion.getByRole("button", { name: "Remove the item Alpha" }).click();
  await completion.getByLabel("New item", { exact: true }).fill("Alpha");
  await completion.getByLabel("New item", { exact: true }).press("Enter");
  await page.getByTestId("node-form-save").click();
  await settled(page);
  await openJourney(page, "browser", "j_bakeoff");
  await openFromCanvas(page, "n_trial_a");
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-checklist")).toHaveAttribute("data-ticked", "0");
  // A refused completion offers the remedy first; the reason field opens only when the bypass is chosen.
  await openActing(page, "browser", "j_bakeoff", "list?frontier=1");
  await frontierItem(page, "n_trial_a").locator("input[type=checkbox]").click();
  await frontierItem(page, "n_trial_b").locator("input[type=checkbox]").click();
  await page.getByTestId("selection-status").click();
  await page.getByRole("menuitem", { name: "Mark done" }).click();
  const refused = page.getByTestId("refused");
  await expect(refused.getByRole("button", { name: /^Tick 1 item and mark both done$/ })).toBeVisible();
  await expect(refused.getByTestId("bypass")).toHaveCount(0);
  await checkpoint(page);
  await refused.getByRole("button", { name: /^Mark both done without/ }).click();
  const reason = refused.getByLabel("Why (optional)");
  // The reason is optional: the button is enabled with the field empty, and a typed one is sent.
  await expect(refused.getByRole("button", { name: "Mark both done anyway" })).toBeEnabled();
  await reason.fill("Checked in person");
  await checkpoint(page);
  await refused.getByRole("button", { name: "Mark both done anyway" }).click();
  await expect(refused).toHaveCount(0);
  await goTo(page, "graph");
  await expect(nodeCard(page, "n_trial_a").getByTestId("card-state")).toHaveAttribute("data-state", "done");
});

test("work that waits is started and finished, and a decision that waits is answered, each as a plain move (F6, D4)", async ({ page }) => {
  let panel = await openNode(page, "browser", "j_bakeoff", "n_comparison");
  await expect(state(panel)).toContainText(/blocked/i);
  await panel.getByRole("button", { name: "Start", exact: true }).click();
  await settled(page);
  await expect(state(panel)).toContainText(/active/i);
  await expect(panel.getByTestId("refused")).toHaveCount(0);
  // A milestone whose prerequisites are open is marked done from its own button.
  panel = await openNode(page, "browser", "j_launch", "n_go_no_go");
  await panel.getByRole("button", { name: "Mark done", exact: true }).click();
  await settled(page);
  await expect(state(panel)).toContainText(/done/i);
  await checkpoint(page);
  panel = await openNode(page, "browser", "j_bakeoff", "n_winner");
  await panel.getByRole("radio").first().check();
  await panel.getByRole("button", { name: "Save", exact: true }).click();
  await settled(page);
  await expect(state(panel)).toContainText(/decided/i);
  await checkpoint(page);
});

