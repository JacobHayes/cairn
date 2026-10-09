// Brief 8.7, Acceptance: the inspector over the in-browser host (each page load seeds the
// fixtures afresh), and over the server host where a view follows another page's edit.
// C8 and its explanations (F7, Gating, Priority, E2), B2's answer form, F5's inline resolution,
// E3's routing, G1 and G2's notes, links, and artifacts with D4's stale and bypass, A10 and G3's
// drafts, J4's history, and a draft across a reload.
import { expect, test } from "@playwright/test";

import { startJourney } from "./around.ts";
import { annotate, dateChain, flag, openNode, pin, section, state } from "./detail.ts";
import { fresh, menuItem, nodePanel, openAt, openFromCanvas, openJourney, recentSaves, syncChip } from "./shell.ts";

test("a decision is answered from its form: nothing preselected, Save waits for a pick, each choice says what it does (B2)", async ({ page }) => {
  const journey = await startJourney(page, "browser", fresh("Answering"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", journey, "plan/graph", { node: "n_partner_runs" });
  const panel = nodePanel(page, "n_partner_runs");
  const save = panel.getByRole("button", { name: "Save", exact: true });
  await expect(panel.getByRole("radio", { checked: true })).toHaveCount(0);
  await expect(save).toBeDisabled();
  await panel.getByRole("button", { name: "Assign owner" }).click();
  await panel.getByRole("menuitem", { name: "Evaluation Lead" }).click();
  await expect(panel.getByTestId("detail-meta")).toContainText("Evaluation Lead");
  await panel.getByRole("radio", { name: /^yes\b/i }).check();
  await save.click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await expect(panel.getByRole("button", { name: "Save change" })).toBeDisabled();
});

test("the final report's inspector reads its due chain, what blocks it, why it ranks, and its history (C8, F7, J4)", async ({ page }) => {
  // A settled not-relevant node is hidden unless asked for; the last step opens one.
  await openJourney(page, "browser", "j_vendor_eval", "?detail=all&show=notrelevant%2Cconditional");
  const panel = await openFromCanvas(page, "n_final_report");
  const due = await dateChain(panel, "Due");
  await expect(due).toHaveAttribute("data-origin", "pin");
  await expect(due.getByTestId("chain-fixed")).toHaveCount(1);
  await expect(due.getByTestId("chain-edit")).toBeVisible();
  const latest = await dateChain(panel, "Latest start");
  await expect(latest).toHaveAttribute("data-origin", "derived");
  expect(await latest.getByTestId("chain-link").count()).toBeGreaterThan(0);
  await expect(state(panel)).toHaveAttribute("data-status", "blocked");
  await expect((await section(panel, "connections")).getByTestId("blocked-by")).toBeVisible();
  await expect((await section(panel, "rank")).getByTestId("gravity")).toBeVisible();
  await expect((await section(panel, "participations")).getByTestId("participation")).not.toHaveCount(0);
  const history = await section(panel, "history");
  await expect(history.getByTestId("history-patch").first()).toBeVisible();
  // The next node opens at its header, not part-way down this one's long body.
  const column = page.locator('[data-pane="inspector"]');
  await column.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  expect(await column.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  // A node that does not apply says why, with the decision as a link, and offers no form.
  const results = await openFromCanvas(page, "n_partner_results");
  await expect.poll(() => column.evaluate((element) => element.scrollTop)).toBe(0);
  await expect(state(results)).toHaveAttribute("data-status", "not_relevant");
  await expect(results.getByTestId("detail-sentence").getByRole("link")).toBeVisible();
  await expect(results.getByTestId("actions").getByRole("button", { name: /^(Mark done|Start|Save)/ })).toHaveCount(0);
});

test("a later pin is rejected with its chain and resolved with a listed move (F5)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  const conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await expect(conflict.getByTestId("chain")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-state", "not-saved");
  await expect(panel.getByTestId("pin-date")).toHaveText("Nov 2");
  await conflict.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(conflict).toHaveCount(0);
  await expect(panel.getByTestId("pin-date")).toHaveText("Nov 20");
  await expect(panel.getByTestId("receipt").first()).toHaveText(/Saved/);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("a milestone's pin is edited through the decision that feeds it (E3)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_decision_meeting");
  await section(panel, "dates");
  await expect(panel.getByTestId("pin-through")).toBeVisible();
  await pin(panel, "2026-11-27");
  await expect(panel.getByTestId("pin-date")).toHaveText("Nov 27");
  await page.locator('[data-testid="pin-through"] a').click();
  const decision = page.locator('[data-testid="node-detail"][data-node="n_meeting_date"]');
  await expect(decision.getByTestId("answer")).toHaveValue("2026-11-27");
  await page.goBack();
  await panel.getByRole("button", { name: /^Unpin/ }).click();
  await expect(panel.getByTestId("pin-date")).toHaveText("No pin");
});

test("a designated artifact completes the node; removing it leaves it done and stale (G1, G2, D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_launch", "n_docs");
  const link = await annotate(panel, "reference", "https://example.org/docs");
  await link.getByRole("button", { name: "Make it the artifact" }).click();
  await expect(panel.locator('[data-testid="annotation"][data-type="artifact"]')).toHaveCount(1);
  await panel.getByTestId("actions").getByRole("button", { name: "Mark done" }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await panel.locator('[data-testid="annotation"][data-type="artifact"]').getByRole("button", { name: "Remove" }).click();
  await expect(flag(panel, "stale")).toBeVisible();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
});

test("a failed guard disables Mark done, and finishing anyway takes a reason (D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await expect(panel.getByTestId("actions").getByRole("button", { name: "Mark done" })).toBeDisabled();
  await expect(state(panel)).toHaveAttribute("data-status", "blocked");
  await menuItem(panel, "done-anyway");
  const bypass = panel.getByTestId("bypass");
  await expect(bypass.getByRole("button", { name: /^Mark done without/ })).toBeDisabled();
  await bypass.getByLabel("Why bypass the guard").fill("Reviewed out of band");
  await bypass.getByRole("button", { name: /^Mark done without/ }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
});

test("a note is added, edited, and removed, attributed and timestamped (G1)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_hiring", "n_offer");
  const note = await annotate(panel, "note", "Check the **start date** first.");
  await expect(note.locator("strong")).toHaveText("start date");
  await note.getByRole("button", { name: "Edit" }).click();
  await panel.getByTestId("annotation-editor").getByLabel("Note").fill("Start date confirmed.");
  await panel.getByTestId("annotation-editor").getByRole("button", { name: "Save" }).click();
  await expect(panel.getByTestId("annotation")).toContainText("Start date confirmed.");
  await expect(panel.getByTestId("annotation")).toContainText("edited");
  await panel.getByTestId("annotation").getByRole("button", { name: "Remove" }).click();
  await expect(panel.getByTestId("annotation")).toHaveCount(0);
});

test("a message draft renders with the journey's context and copies (A10, G3)", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_access");
  const name = (await page.getByTestId("journey-name").textContent()) ?? "";
  const draft = (await section(panel, "resources")).getByTestId("draft-text");
  await expect(draft).toContainText(name);
  await expect(draft.getByTestId("draft-missing")).toHaveCount(0);
  await panel.getByTestId("message-draft").getByRole("button", { name: "Copy" }).click();
  await expect(panel.getByTestId("copied")).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(await draft.innerText());
});

test("a rejection and an unsent bypass reason survive a reload (D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await menuItem(panel, "done-anyway");
  const reason = fresh("Reviewed out of band");
  await panel.getByTestId("bypass").getByLabel("Why bypass the guard").fill(reason);
  await page.reload();
  await expect(page.getByTestId("node-detail").getByTestId("bypass").getByLabel("Why bypass the guard")).toHaveValue(reason);
});

test("over the server, a resolution never overwrites a pin someone set meanwhile (F5, H5)", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const mine = await openNode(one, "server", "j_vendor_eval", "n_final_report");
  const theirs = await openNode(two, "server", "j_vendor_eval", "n_final_report");
  await pin(mine, "2026-11-25");
  await expect(mine.getByTestId("date-conflict")).toBeVisible();
  await pin(theirs, "2026-11-10");
  await expect(theirs.getByTestId("pin-date")).toHaveText("Nov 10");
  await expect(mine.getByTestId("pin-date")).toHaveText("Nov 10");
  await mine.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(mine.getByTestId("conflict")).toBeVisible();
  await expect(theirs.getByTestId("pin-date")).toHaveText("Nov 10");
});

test("work whose relevance waits on an unanswered decision completes with a warning (D4, D7)", async ({ page }) => {
  const offer = await openNode(page, "browser", "j_hiring", "n_make_offer");
  await menuItem(offer, "reopen");
  await expect(state(offer)).toHaveAttribute("data-status", "ready");
  const panel = await openFromCanvas(page, "n_close_out");
  await expect(panel.getByTestId("detail-sentence").getByRole("link", { name: "Make an offer" })).toBeVisible();
  await panel.getByTestId("actions").getByRole("button", { name: "Mark done" }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await expect(panel.getByTestId("actions").getByTestId("receipt-warning")).toContainText("May not apply");
  // The chip and Recent say it was saved; nothing pops up over the page.
  expect((await recentSaves(page))[0]).toContain("May not apply");
  await expect(page.getByTestId("toast")).toHaveCount(0);
  await expect((await section(panel, "connections")).getByTestId("relevance-why")).toHaveAttribute("data-status", "undecided");
});
