// The acting surfaces in Chromium (rung 6): the list's filters, grouping, search, and bulk
// actions as one patch (C9); the next list's ranking, re-sort, rows, folds and stalled panel
// (C10, D5); the cards' pass and the inspector component they hold (C11, B10); snoozes leaving
// and returning (B6); Mine across journeys (C16); and the decision walkthrough over a fresh
// journey started from the vendor evaluation's route (C11, D2). Every test runs on the
// in-browser host, whose fixtures are fresh on every load.
import { expect, test, type Page } from "@playwright/test";

import {
  answerCard,
  card,
  daysAfter,
  listKeys,
  nextItem,
  nextKeys,
  openActing,
  passOrder,
  publishFollowUpRoute,
  revisionAfter,
  select,
  turnOn,
} from "./acting.ts";
import { journeyName, startJourney } from "./around.ts";
import { section } from "./detail.ts";
import { derivedRevision, goTo, goWithin, menuItem, nodePanel, openAt, syncChip } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

/** Starts a fresh journey from version 1 of the vendor evaluation's route (B1); its id. */
const startVendorJourney = (page: Page) => startJourney(page, "browser", journeyName("Walkthrough"), { route: "vendor-evaluation", version: 1 });

const UP_FRONT = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];

/** The product launch's acting frontier at the end of its scenario in rank order, read on `FIXED_TODAY`. */
const LAUNCH_RANKED = ["n_launch", "n_docs", "n_announcement", "n_beta_end", "n_retro"];

/** Holds the page's clock at noon UTC on `FIXED_TODAY`, so the in-browser host derives with that today. */
async function atFixedToday(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
}

test("C10: the next list ranks the frontier, and re-sorting by slack reorders it", async ({ page }) => {
  await atFixedToday(page);
  await openActing(page, "browser", "j_launch", "next/list");
  // At the scenario matrix's day every slack is past the urgency window, so gravity and
  // leverage rank: the launch leads, then the work feeding it; the retrospective, with no
  // deadline, sorts last by slack.
  expect(await nextKeys(page)).toEqual(LAUNCH_RANKED);
  const ranks = await page.getByTestId("next-item").evaluateAll((items) => items.map((item) => Number(item.getAttribute("data-rank"))));
  expect(ranks).toEqual([...ranks].sort((left, right) => right - left));
  await page.getByLabel("Sort by").selectOption("slack");
  await expect.poll(() => nextKeys(page)).toEqual(["n_docs", "n_announcement", "n_beta_end", "n_launch", "n_retro"]);
  const slacks = await page.getByTestId("next-item").evaluateAll((items) => items.map((item) => item.getAttribute("data-slack") ?? ""));
  const known = slacks.filter((slack) => slack !== "").map(Number);
  expect(known).toEqual([...known].sort((left, right) => left - right));
  expect(slacks.slice(known.length).every((slack) => slack === "")).toBe(true);
  const fold = page.getByTestId("fold-look");
  await expect(fold).toHaveAttribute("data-count", "2");
  await expect(fold.getByTestId("fold-item").first()).toBeHidden();
  await fold.locator("summary").click();
  await expect(fold.locator('[data-testid="fold-item"][data-node="n_launch"]')).toHaveAttribute("data-reason", "open");
});

test("C9: filters hold at once, and search reads notes", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list?flag=next_up");
  expect((await listKeys(page)).sort()).toEqual(["n_decision_meeting", "n_review_opens"]);
  await turnOn(page, "kind-decision");
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
  await openActing(page, "browser", "j_vendor_eval", "plan/list");
  await page.getByRole("searchbox", { name: "Search" }).fill("environment team");
  await page.keyboard.press("Enter");
  await expect.poll(() => listKeys(page)).toEqual(["n_access"]);
});

test("C9: the list is a tree folded to its top level, sorting a column flattens it, and not-relevant rows stay out until asked for", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list");
  const depths = () => page.getByTestId("list-row").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-depth")));
  expect(new Set(await depths())).toEqual(new Set(["0"]));
  await page.getByTestId("list-fold-all").click();
  await expect(page.locator('[data-testid="list-row"][data-node="n_plan_draft"]')).toHaveAttribute("data-depth", "2");
  await page.getByRole("button", { name: "Sort by due" }).click();
  await expect.poll(async () => new Set(await depths())).toEqual(new Set(["0"]));
  await expect(page.locator('[data-testid="list-row"][data-node="n_plan_draft"] [data-testid="breadcrumb"]')).toContainText("Test plan");
  const total = page.getByTestId("list-total");
  await expect(page.getByTestId("tab-plan-count")).toHaveText((await total.getAttribute("data-total")) ?? "");
  expect(await listKeys(page)).not.toContain("n_partner_results");
  await page.getByTestId("list-not-relevant").click();
  await expect.poll(() => listKeys(page)).toContain("n_partner_results");
});

test("C9: a bulk completion with one node failing its guard is rejected whole, naming it", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list?sort=rank");
  const revision = await derivedRevision(page);
  await select(page, "n_final_report");
  await select(page, "n_decision_meeting");
  await page.getByTestId("bulk-bar").getByRole("button", { name: "Done", exact: true }).click();
  const failed = page.locator('[data-testid="violation"][data-code="guard_failed"]');
  await expect(failed).toHaveAttribute("data-node", "n_final_report");
  await expect(failed).toContainText("Final report");
  // A later write over the same selection settles whatever the rejected action sent: only it
  // may move the revision, and the milestone the rejected patch would have reached is still ready.
  await page.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Assign owner..." }).click();
  await page.getByLabel("Owner for them").selectOption("e_lead");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await expect(page.locator('[data-testid="list-row"][data-node="n_decision_meeting"]')).toContainText("ready");
});

test("C9, B6: a bulk snooze is one patch; the snoozed leave the next list and an unsnooze returns them", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "plan/list?flag=next_up");
  const revision = await derivedRevision(page);
  await select(page, "n_docs");
  await select(page, "n_announcement");
  await page.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Snooze until a node..." }).click();
  await page.getByLabel("Snooze them until node").selectOption("n_beta_end");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await goTo(page, "next", "list");
  await expect(nextItem(page, "n_beta_end")).toBeVisible();
  await expect(nextItem(page, "n_docs")).toHaveCount(0);
  await expect(nextItem(page, "n_announcement")).toHaveCount(0);
  const fold = page.getByTestId("fold-snoozed");
  await expect(fold).toHaveAttribute("data-count", "2");
  await fold.locator("summary").click();
  await expect(fold.getByTestId("snooze-group")).toHaveCount(1);
  await fold.locator('[data-testid="fold-item"][data-node="n_docs"]').getByRole("button", { name: "Unsnooze" }).click();
  await expect(nextItem(page, "n_docs")).toBeVisible();
  await goTo(page, "plan", "list");
  await select(page, "n_announcement");
  await page.getByTestId("bulk-bar").getByRole("button", { name: "Unsnooze" }).click();
  expect(await revisionAfter(page, revision + 2)).toBe(revision + 3);
  await goTo(page, "next", "list");
  await expect(nextItem(page, "n_announcement")).toBeVisible();
});

test("B6: a node snoozed from its card leaves, and returns when its target completes", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "next/cards");
  const first = (await passOrder(page))[0] ?? "";
  await expect(card(page)).toHaveAttribute("data-node", first);
  const target = first === "n_beta_end" ? "n_retro" : "n_beta_end";
  await menuItem(card(page), "snooze");
  await card(page).getByLabel("When something is done").check();
  await card(page).getByLabel("Snooze until node").selectOption(target);
  await card(page).getByTestId("snooze").getByRole("button", { name: "Snooze", exact: true }).click();
  await expect.poll(() => passOrder(page)).not.toContain(first);
  await goTo(page, "next", "list");
  await nextItem(page, target).getByRole("button", { name: "Mark reached" }).click();
  await expect(nextItem(page, first)).toBeVisible();
});

test("D5: an empty acting frontier shows the stalled panel, and unsnooze brings the node back", async ({ page }) => {
  await openActing(page, "browser", "j_hiring", "next/list");
  expect(await nextKeys(page)).toEqual(["n_offer"]);
  const today = (await syncChip(page).getAttribute("data-today")) ?? "";
  await nextItem(page, "n_offer").getByRole("link", { name: "Offer letter" }).click();
  const panel = nodePanel(page, "n_offer");
  await menuItem(panel, "snooze");
  await panel.getByLabel("A date").check();
  await panel.getByLabel("Snooze until", { exact: true }).fill(daysAfter(today, 7));
  await panel.getByTestId("snooze").getByRole("button", { name: "Snooze", exact: true }).click();
  const cause = page.locator('[data-testid="stall-cause"][data-status="snooze"]');
  await expect(cause).toHaveAttribute("data-node", "n_offer");
  await expect(page.getByTestId("next-for-you")).toHaveCount(0);
  await expect(page.getByTestId("fold-snoozed")).toHaveAttribute("data-count", "1");
  await cause.getByRole("button", { name: "Unsnooze" }).click();
  await expect(nextItem(page, "n_offer")).toBeVisible();
  await expect(page.getByTestId("stalled")).toHaveCount(0);
});

test("C11: a card is the inspector's panel with Pass apart; pass writes nothing, a rail link swaps the rail for its inspector, and a finished pass goes round again", async ({ page }) => {
  await atFixedToday(page);
  await openActing(page, "browser", "j_launch", "next/cards");
  const revision = await derivedRevision(page);
  const detail = card(page).getByTestId("node-detail");
  await expect(detail.getByRole("button", { name: "Mark reached" })).toBeVisible();
  await expect(detail.getByTestId("pass")).toHaveCount(0);
  const rail = page.getByTestId("pass-rail");
  await rail.getByRole("link", { name: "Documentation" }).click();
  await expect(nodePanel(page, "n_docs")).toBeVisible();
  await expect(rail).toHaveCount(0);
  await page.getByTestId("back-to-pass").click();
  await expect(rail).toBeVisible();
  const [first = "", second = ""] = await passOrder(page);
  await card(page).getByTestId("pass").click();
  await expect(card(page)).toHaveAttribute("data-node", second);
  expect((await passOrder(page)).at(-1)).toBe(first);
  await page.keyboard.press("p");
  await expect(card(page)).not.toHaveAttribute("data-node", second);
  expect(await derivedRevision(page)).toBe(revision);
  for (let at = 2; at < LAUNCH_RANKED.length; at += 1) {
    await card(page).getByTestId("pass").click();
  }
  const done = page.getByTestId("pass-done");
  await expect(done).toBeVisible();
  await done.getByRole("button", { name: "Go round again" }).click();
  await expect(card(page)).toBeVisible();
});

test("C10: rows keep their order under hover and selection", async ({ page }) => {
  const journey = await startJourney(page, "browser", journeyName("Order"), { route: "vendor-evaluation", version: 1 });
  await openActing(page, "browser", journey, "next/list");
  const order = await nextKeys(page);
  await nextItem(page, order[0] ?? "").hover();
  expect(await nextKeys(page)).toEqual(order);
  await nextItem(page, order[2] ?? "").click();
  await expect(nodePanel(page, order[2] ?? "")).toBeVisible();
  await page.keyboard.press("j");
  await expect(nodePanel(page, order[3] ?? "")).toBeVisible();
  expect(await nextKeys(page)).toEqual(order);
});

test("C11: the walkthrough opens on the decisions at the start; answering the partner decision surfaces its work in the same pass", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "next/cards?decisions=1");
  await expect(page.getByTestId("walkthrough-intro")).toBeVisible();
  expect((await passOrder(page)).sort()).toEqual(UP_FRONT);
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await answerCard(page, "yes");
  await expect(page.getByTestId("surfaced").locator('[data-node="n_criteria"]')).toBeVisible();
  expect((await passOrder(page)).sort()).toEqual(UP_FRONT.filter((key) => key !== "n_partner_runs"));
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => passOrder(page)).toContain("n_criteria");
  // Every kind holds the journey's acting frontier: kickoff, the decision meeting, the
  // decisions still open, and the partner-led work the answer surfaced; the next list keeps
  // the shared rank, which the pass's order for it (surfaced work first) does not change.
  const pass = await passOrder(page);
  const everyKind = ["n_criteria", "n_decision_meeting", "n_kickoff", ...UP_FRONT.filter((key) => key !== "n_partner_runs")];
  expect([...pass].sort()).toEqual(everyKind.sort());
  await goTo(page, "next", "list");
  expect(await nextKeys(page)).toEqual(["n_kickoff", "n_criteria", ...pass.slice(2)]);
});

test("C11: what an answer unlocked comes next in the pass, labeled; passing sends it to the back, and a node completed from the pass unlocks into it too", async ({ page }) => {
  const route = await publishFollowUpRoute(page);
  await startJourney(page, "browser", journeyName("Follow-up"), { route, version: 1 });
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await answerCard(page, "yes");
  // The follow-up ranks below the up-front decisions still open, and still comes first.
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_scope");
  await expect(page.getByTestId("unlocked-by").getByRole("link", { name: "Partner runs testing" })).toBeVisible();
  await card(page).getByTestId("pass").click();
  await expect(page.getByTestId("unlocked-by")).toHaveCount(0);
  expect((await passOrder(page)).at(-1)).toBe("n_partner_scope");
  // Every kind: the partner decision's other unlock is next, until a new pass returns to rank order.
  await page.getByTestId("chip-decisions").click();
  await expect(card(page)).toHaveAttribute("data-node", "n_criteria");
  await page.getByTestId("new-pass").click();
  await expect(card(page)).toHaveAttribute("data-node", "n_kickoff");
  await expect(page.getByTestId("unlocked-by")).toHaveCount(0);
  // Completed in its own inspector, opened from the pass, the action unlocks what waits on it.
  await page.getByTestId("pass-rail").getByRole("link", { name: "Review the partner's criteria" }).click();
  await nodePanel(page, "n_criteria").getByTestId("actions").getByRole("button", { name: "Mark done" }).click();
  await page.getByTestId("back-to-pass").click();
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_results");
  await expect(page.getByTestId("unlocked-by").getByRole("link", { name: "Review the partner's criteria" })).toBeVisible();
});

test("B2, C8, C12: a rationale given on a triage card shows in node detail; a revision without one drops it", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "next/cards?decisions=1");
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await answerCard(page, "yes", "A partner brings the domain.");
  await openAt(page, "browser", journey, "plan/graph?decisions=1", { node: "n_partner_runs" });
  const panel = nodePanel(page, "n_partner_runs");
  await expect(panel.getByTestId("rationale")).toContainText("A partner brings the domain.");
  // A new answer starts with no reason: the old one is offered, never carried forward.
  const editor = panel.getByTestId("answer-editor");
  await editor.getByRole("radio", { name: /^no\b/i }).check();
  await expect(editor.getByLabel("Why")).toHaveValue("");
  await editor.getByRole("button", { name: "Save change" }).click();
  await expect(panel.getByTestId("rationale")).toHaveCount(0);
});

test("G4, C11, D4: done on an action that requires a note completes in one patch; losing the note later is listed to fix", async ({ page }) => {
  const journey = await startJourney(page, "browser", journeyName("Screening"), { route: "hiring-loop", version: 1 });
  await openActing(page, "browser", journey, "next/cards");
  await expect(card(page).getByTestId("actions").getByRole("button", { name: "Done...", exact: true })).toBeVisible();
  await goTo(page, "next", "list");
  const item = nextItem(page, "n_screen");
  await item.getByRole("button", { name: "Done...", exact: true }).click();
  const save = item.getByRole("button", { name: "Add note and mark done" });
  await expect(save).toBeDisabled();
  const revision = await derivedRevision(page);
  await item.getByLabel("Note").fill("Covered the role and the timeline.");
  await save.click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await expect(nextItem(page, "n_screen")).toHaveCount(0);
  // Finished work that later loses its note is listed under needs a look, with its fix beside it.
  await goWithin(page, `/journeys/${journey}/next/list/nodes/n_screen`);
  const notes = await section(nodePanel(page, "n_screen"), "annotations");
  await notes.getByTestId("annotation").getByRole("button", { name: "Remove" }).click();
  const fold = page.getByTestId("fold-look");
  await fold.locator("summary").click();
  const stale = fold.locator('[data-testid="fold-item"][data-node="n_screen"]');
  await expect(stale).toHaveAttribute("data-reason", "note");
  await stale.getByRole("button", { name: "Add note" }).click();
  await stale.getByLabel("Note").fill("Covered the role.");
  await stale.getByRole("button", { name: "Save note" }).click();
  await expect(fold).toHaveCount(0);
});

test("C11, B10: a placeholder's card offers mark atomic, and no more once it is atomic", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "next/cards");
  await expect(card(page)).toHaveAttribute("data-node", "n_kickoff");
  await card(page).getByRole("button", { name: "Mark reached" }).click();
  await openActing(page, "browser", journey, "next/cards?kind=deliverable");
  await expect.poll(() => passOrder(page)).toContain("n_workload");
  while ((await card(page).getAttribute("data-node")) !== "n_workload") {
    await card(page).getByTestId("pass").click();
  }
  await card(page).getByRole("button", { name: "Mark atomic" }).click();
  await expect(card(page).getByRole("button", { name: "Mark atomic" })).toHaveCount(0);
});

test("C11, C9: with every decision skipped in bulk, the walkthrough shows what would unblock the next ones", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "plan/list?flag=decisions_needed");
  await expect.poll(() => listKeys(page)).toHaveLength(UP_FRONT.length);
  await page.getByLabel("Select every row shown").click();
  await page.getByRole("button", { name: "Skip..." }).click();
  await page.getByLabel("Why skip them").fill("decided elsewhere");
  await page.getByRole("button", { name: `Apply to ${String(UP_FRONT.length)}` }).click();
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
  await goTo(page, "next", "cards");
  await page.getByTestId("chip-decisions").click();
  const comparison = page.locator('[data-testid="waiting-decision"][data-node="n_comparison_set"]');
  await expect(comparison.locator('[data-testid="unblocker"][data-node="n_plan"]')).toBeVisible();
  await page.getByRole("button", { name: "Continue with everything" }).click();
  await expect(page.getByTestId("chip-decisions")).toHaveAttribute("aria-pressed", "false");
  await expect.poll(() => passOrder(page)).toContain("n_kickoff");
});

test("C11: a walkthrough filtered to only mine says the open decisions are others', not that none can be made", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "next/cards?decisions=1&mine=1");
  await expect(page.getByTestId("triage-empty")).toHaveAttribute("data-status", "filtered");
  await expect(page.getByTestId("waiting-decisions")).toHaveCount(0);
});
