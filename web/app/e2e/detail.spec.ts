// Brief 5.1, Acceptance: the node detail panel over the in-browser host (each page load seeds
// the fixtures afresh), and over the server host where a view follows another page's edit.
// C8 and its explanations (F7, Gating, Priority, E2), F5's inline resolution, E3's routing,
// G1 and G2's notes, links, and artifacts with D4's stale and bypass, A10 and G3's drafts,
// J4's history, drafts across a reload, and the panel on a narrow screen.
import { expect, test } from "@playwright/test";

import { annotate, dateChain, flag, openNode, pin, section, state } from "./detail.ts";
import { fresh, live, nodePanel, openAt, openFromCanvas, recentSaves, syncChip } from "./shell.ts";

test("the final report's detail reads its due chain and what to edit (C8, F7)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  const due = await dateChain(panel, "Due");
  await expect(due).toHaveAttribute("data-origin", "pin");
  await expect(due.getByTestId("chain-fixed")).toHaveCount(1);
  await expect(due.getByTestId("chain-edit")).toBeVisible();
  const latest = await dateChain(panel, "Latest start");
  await expect(latest).toHaveAttribute("data-origin", "derived");
  expect(await latest.getByTestId("chain-link").count()).toBeGreaterThan(0);
  await expect(state(panel)).toHaveAttribute("data-status", "blocked");
  await expect((await section(panel, "blocking")).getByTestId("blocked-through")).toBeVisible();
  await expect((await section(panel, "priority")).getByTestId("gravity")).toBeVisible();
  await expect((await section(panel, "participations")).getByTestId("participation")).not.toHaveCount(0);
});

test("relevance names the decision that produced it (C8, Gating)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_partner_results");
  const relevance = await section(panel, "relevance");
  await expect(relevance.getByTestId("relevance-why")).toHaveAttribute("data-status", "not_relevant");
  await expect(relevance.getByRole("link").first()).toBeVisible();
  await expect(state(panel)).toHaveAttribute("data-status", "not_relevant");
  await expect(state(panel)).toHaveText("not relevant");
});

test("priority lists gravity's contributors and leverage split by owner (C8, Priority)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_bakeoff", "n_comparison");
  const priority = await section(panel, "priority");
  await expect(priority.getByTestId("gravity-from").locator("li")).not.toHaveCount(0);
  await expect(priority.getByTestId("leverage-other").locator("li")).not.toHaveCount(0);
});

test("a later pin is rejected with its chain and resolved with a listed move (F5)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  const conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await expect(conflict.getByTestId("chain")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-state", "not-saved");
  await expect(panel.getByTestId("pin-date")).toHaveText("2026-11-02");
  await conflict.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(conflict).toHaveCount(0);
  await expect(panel.getByTestId("pin-date")).toHaveText("2026-11-20");
  await expect(panel.getByTestId("receipt").first()).toHaveText(/Saved/);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("a milestone's pin is edited through the decision that feeds it (E3)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_decision_meeting");
  await expect(panel.getByTestId("pin-through")).toBeVisible();
  await pin(panel, "2026-11-27");
  await expect(panel.getByTestId("pin-date")).toHaveText("2026-11-27");
  await page.locator('[data-testid="pin-through"] a').click();
  const decision = page.locator('[data-testid="node-detail"][data-node="n_meeting_date"]');
  await expect(decision.getByTestId("answer")).toContainText("2026-11-27");
  await page.goBack();
  await panel.getByRole("button", { name: /^Unpin/ }).click();
  await expect(panel.getByTestId("pin-date")).toHaveText("none");
});

test("a designated artifact completes the node; removing it leaves it done and stale (G1, G2, D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_launch", "n_docs");
  const link = await annotate(panel, "reference", "https://example.org/docs");
  await link.getByRole("button", { name: "Make it the artifact" }).click();
  await expect(panel.locator('[data-testid="annotation"][data-type="artifact"]')).toHaveCount(1);
  await panel.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await panel.locator('[data-testid="annotation"][data-type="artifact"]').getByRole("button", { name: "Remove" }).click();
  await expect(flag(panel, "stale")).toBeVisible();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await expect(panel.getByTestId("stale-reason")).toHaveCount(1);
});

test("a failed guard is shown and bypassed with a reason (D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await panel.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  const bypass = panel.getByTestId("bypass");
  await expect(bypass).toBeVisible();
  await expect(state(panel)).toHaveAttribute("data-status", "blocked");
  await bypass.getByLabel("Why bypass the guard").fill("Reviewed out of band");
  await bypass.getByRole("button", { name: "Bypass" }).click();
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
  const draft = panel.getByTestId("draft-text");
  await expect(draft).toContainText(name);
  await expect(draft.getByTestId("draft-missing")).toHaveCount(0);
  await panel.getByTestId("message-draft").getByRole("button", { name: "Copy" }).click();
  await expect(panel.getByTestId("copied")).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(await draft.innerText());
});

test("history pages the node's events, grouped by patch (J4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_access");
  await panel.getByTestId("history").locator("summary").click();
  await expect(panel.getByTestId("history-patch").first()).toBeVisible();
});

test("an unsent note survives a reload", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_bakeoff", "n_summary");
  await panel.getByRole("button", { name: "Add a note or link" }).click();
  const text = fresh("Unsent note");
  await panel.getByLabel("Note").fill(text);
  await page.reload();
  await expect(page.getByTestId("node-detail").getByLabel("Note")).toHaveValue(text);
});

test("an unsent note stays with its node when another node is opened", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await panel.getByRole("button", { name: "Add a note or link" }).click();
  await panel.getByLabel("Note").fill(fresh("For the report"));
  await panel.getByRole("link", { name: "Final review" }).first().click();
  const other = page.locator('[data-testid="node-detail"][data-node="n_final_review"]');
  await expect(other).toBeVisible();
  await expect(other.getByTestId("annotation-editor")).toHaveCount(0);
});

test("an unsent skip reason survives a reload", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_hiring", "n_offer");
  await panel.getByTestId("actions").getByRole("button", { name: "Skip" }).click();
  const reason = fresh("Not needed");
  await panel.getByLabel("Why skip it").fill(reason);
  await page.reload();
  await expect(page.getByTestId("node-detail").getByLabel("Why skip it")).toHaveValue(reason);
});

test("a rejection and an unsent bypass reason survive a reload (D4)", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await panel.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  const reason = fresh("Reviewed out of band");
  await panel.getByTestId("bypass").getByLabel("Why bypass the guard").fill(reason);
  await page.reload();
  await expect(page.getByTestId("node-detail").getByTestId("bypass").getByLabel("Why bypass the guard")).toHaveValue(reason);
});

test("on a narrow screen the panel follows the journey's header and toolbar, with nothing off the side", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await openAt(page, "browser", "j_vendor_eval", "plan/graph", { node: "n_final_report" });
  const panel = nodePanel(page, "n_final_report");
  await expect(panel).toBeVisible();
  const [panelBox, toolbarBox] = [await panel.boundingBox(), await page.getByTestId("journey-toolbar").boundingBox()];
  expect((toolbarBox?.y ?? 0) < (panelBox?.y ?? 0)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test("over the server, a resolution never overwrites a pin someone set meanwhile (F5, H5)", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const mine = await openNode(one, "server", "j_vendor_eval", "n_final_report");
  const theirs = await openNode(two, "server", "j_vendor_eval", "n_final_report");
  await pin(mine, "2026-11-25");
  await expect(mine.getByTestId("date-conflict")).toBeVisible();
  await pin(theirs, "2026-11-10");
  await expect(theirs.getByTestId("pin-date")).toHaveText("2026-11-10");
  await expect(mine.getByTestId("pin-date")).toHaveText("2026-11-10");
  await mine.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(mine.getByTestId("conflict")).toBeVisible();
  await expect(theirs.getByTestId("pin-date")).toHaveText("2026-11-10");
});

test("over the server, a note added in one page appears in another's panel (H6)", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  const mine = await openNode(one, "server", "j_hiring", "n_close_out");
  const theirs = await openNode(two, "server", "j_hiring", "n_close_out");
  await live(two);
  const text = fresh("Called the candidate");
  await annotate(mine, "note", text);
  await expect(theirs.getByTestId("annotations")).toContainText(text);
});

test("work whose relevance waits on an unanswered decision completes with a warning (D4, D7)", async ({ page }) => {
  const offer = await openNode(page, "browser", "j_hiring", "n_make_offer");
  await offer.getByTestId("actions").getByRole("button", { name: "Reopen" }).click();
  await expect(state(offer)).toHaveAttribute("data-status", "ready");
  const panel = await openFromCanvas(page, "n_close_out");
  const warning = panel.getByTestId("actions").getByTestId("may-not-apply");
  await expect(warning).toHaveAttribute("data-unanswered", "n_make_offer");
  await panel.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await expect(panel.getByTestId("actions").getByTestId("receipt-warning")).toContainText("May not apply");
  // The chip and Recent say it was saved; nothing pops up over the page.
  expect((await recentSaves(page))[0]).toContain("May not apply");
  await expect(page.getByTestId("toast")).toHaveCount(0);
  await expect((await section(panel, "relevance")).getByTestId("relevance-why")).toHaveAttribute("data-status", "undecided");
});
