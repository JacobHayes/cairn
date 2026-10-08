// The acting surfaces in Chromium (rung 6): the list's filters, grouping, search, and bulk
// actions as one patch (C9); the next list's ranking, re-sort, and stalled panel (C10, D5);
// triage's pass and per-kind cards (C11, B10); snoozes leaving and returning (B6); and the
// decision walkthrough over a fresh journey started from the vendor evaluation's route (C11,
// D2). Every test runs on the in-browser host, whose fixtures are fresh on every load.
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
  revisionAfter,
  select,
  turnOn,
} from "./acting.ts";
import { journeyName, startJourney } from "./around.ts";
import { derivedRevision } from "./shell.ts";
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
  await openActing(page, "browser", "j_launch", "next");
  // At the scenario matrix's day every slack is past the urgency window, so gravity and
  // leverage rank: the launch leads, then the work feeding it; the retrospective, with no
  // deadline, sorts last by slack.
  expect(await nextKeys(page)).toEqual(LAUNCH_RANKED);
  await expect(page.getByTestId("breadcrumb").first()).toBeVisible();
  const ranks = await page.getByTestId("why").evaluateAll((whys) => whys.map((why) => Number(why.getAttribute("data-rank"))));
  expect(ranks).toEqual([...ranks].sort((left, right) => right - left));
  await page.getByLabel("Sort by").selectOption("slack");
  await expect.poll(() => nextKeys(page)).toEqual(["n_docs", "n_announcement", "n_beta_end", "n_launch", "n_retro"]);
  const slacks = await page.getByTestId("next-item").evaluateAll((items) => items.map((item) => item.getAttribute("data-slack") ?? ""));
  const known = slacks.filter((slack) => slack !== "").map(Number);
  expect(known).toEqual([...known].sort((left, right) => left - right));
  expect(slacks.slice(known.length).every((slack) => slack === "")).toBe(true);
});

test("C11: triage holds the acting frontier, one card at a time in rank order", async ({ page }) => {
  await atFixedToday(page);
  await openActing(page, "browser", "j_launch", "triage");
  expect(await passOrder(page)).toEqual(LAUNCH_RANKED);
  await expect(card(page)).toHaveAttribute("data-node", LAUNCH_RANKED[0] ?? "");
});

test("C9: filters hold at once, search reads notes, and rows group by container", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "list?flag=next_up");
  expect((await listKeys(page)).sort()).toEqual(["n_decision_meeting", "n_review_opens"]);
  await turnOn(page, "kind-decision");
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
  await openActing(page, "browser", "j_vendor_eval", "list");
  await page.getByRole("searchbox", { name: "Search" }).fill("environment team");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect.poll(() => listKeys(page)).toEqual(["n_access"]);
  await openActing(page, "browser", "j_vendor_eval", "list?kind=action&group=container");
  const groups = await page.getByTestId("list-group").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-node")));
  expect([...groups].sort()).toEqual(["n_partner_led", "n_plan"]);
});

test("C9: a bulk completion with one node failing its guard is rejected whole, naming it", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "list");
  const revision = await derivedRevision(page);
  await select(page, "n_final_report");
  await select(page, "n_decision_meeting");
  await page.getByTestId("bulk-bar").getByRole("button", { name: "Done", exact: true }).click();
  const failed = page.locator('[data-testid="violation"][data-code="guard_failed"]');
  await expect(failed).toHaveAttribute("data-node", "n_final_report");
  await expect(failed).toContainText("Final report");
  // A later write over the same selection settles whatever the rejected action sent: only it
  // may move the revision, and the milestone the rejected patch would have reached is pending.
  await page.getByRole("button", { name: "Assign owner..." }).click();
  await page.getByLabel("Owner for them").selectOption("e_lead");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await expect(page.locator('[data-testid="list-row"][data-node="n_decision_meeting"]')).toContainText("pending");
});

test("C9, B6: a bulk snooze is one patch; the snoozed leave the next list and an unsnooze returns them", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "list?flag=next_up");
  const revision = await derivedRevision(page);
  await select(page, "n_docs");
  await select(page, "n_announcement");
  await page.getByRole("button", { name: "Snooze until a node..." }).click();
  await page.getByLabel("Snooze them until node").selectOption("n_beta_end");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await page.getByTestId("nav-next").click();
  await expect(nextItem(page, "n_beta_end")).toBeVisible();
  await expect(nextItem(page, "n_docs")).toHaveCount(0);
  await expect(nextItem(page, "n_announcement")).toHaveCount(0);
  await page.getByTestId("nav-list").click();
  await select(page, "n_docs");
  await select(page, "n_announcement");
  await page.getByTestId("bulk-bar").getByRole("button", { name: "Unsnooze" }).click();
  expect(await revisionAfter(page, revision + 1)).toBe(revision + 2);
  await page.getByTestId("nav-next").click();
  await expect(nextItem(page, "n_docs")).toBeVisible();
});

test("B6: a node snoozed from its card leaves, and returns when its target completes", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "triage");
  const first = (await passOrder(page))[0] ?? "";
  await expect(card(page)).toHaveAttribute("data-node", first);
  const target = first === "n_beta_end" ? "n_retro" : "n_beta_end";
  await card(page).getByRole("button", { name: "Snooze until a node" }).click();
  await card(page).getByLabel("Snooze until node").selectOption(target);
  await card(page).getByTestId("snooze-node-form").getByRole("button", { name: "Save" }).click();
  await expect.poll(() => passOrder(page)).not.toContain(first);
  await page.getByTestId("nav-next").click();
  await nextItem(page, target).getByRole("button", { name: "Mark reached" }).click();
  await expect(nextItem(page, first)).toBeVisible();
});

test("D5: an empty acting frontier shows the stalled panel, and unsnooze brings the node back", async ({ page }) => {
  await openActing(page, "browser", "j_hiring", "next");
  expect(await nextKeys(page)).toEqual(["n_offer"]);
  const today = (await page.getByTestId("derivation").getAttribute("data-today")) ?? "";
  await nextItem(page, "n_offer").getByRole("button", { name: "Snooze until a date" }).click();
  await nextItem(page, "n_offer").getByLabel("Snooze until").fill(daysAfter(today, 7));
  await nextItem(page, "n_offer").getByTestId("snooze-date-form").getByRole("button", { name: "Save" }).click();
  const cause = page.locator('[data-testid="stall-cause"][data-status="snooze"]');
  await expect(cause).toHaveAttribute("data-node", "n_offer");
  await cause.getByRole("button", { name: "Unsnooze" }).click();
  await expect(nextItem(page, "n_offer")).toBeVisible();
  await expect(page.getByTestId("stalled")).toHaveCount(0);
});

test("C11: pass writes nothing and sends the card to the back of the pass", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "triage");
  const revision = await derivedRevision(page);
  const [first = "", second = ""] = await passOrder(page);
  await card(page).getByTestId("pass").click();
  await expect(card(page)).toHaveAttribute("data-node", second);
  expect((await passOrder(page)).at(-1)).toBe(first);
  await page.keyboard.press("p");
  await expect(card(page)).not.toHaveAttribute("data-node", second);
  expect(await derivedRevision(page)).toBe(revision);
});

test("C11: the walkthrough opens on the decisions at the start; answering the partner decision surfaces its work in the same pass", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "triage?mode=decisions");
  expect((await passOrder(page)).sort()).toEqual(UP_FRONT);
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await card(page).getByLabel("Assign owner").selectOption("e_lead");
  await card(page).getByRole("button", { name: "Assign", exact: true }).click();
  await expect(card(page).locator('[data-testid="flag"][data-status="unassigned"]')).toHaveCount(0);
  await answerCard(page, "yes");
  await expect(page.getByTestId("surfaced").locator('[data-node="n_criteria"]')).toBeVisible();
  expect((await passOrder(page)).sort()).toEqual(UP_FRONT.filter((key) => key !== "n_partner_runs"));
  await page.getByRole("link", { name: "Every kind" }).click();
  await expect.poll(() => passOrder(page)).toContain("n_criteria");
  // Every kind holds the journey's acting frontier: kickoff, the decision meeting, the
  // decisions still open, and the partner-led work the answer surfaced, in the next list's
  // order.
  const pass = await passOrder(page);
  const everyKind = ["n_criteria", "n_decision_meeting", "n_kickoff", ...UP_FRONT.filter((key) => key !== "n_partner_runs")];
  expect([...pass].sort()).toEqual(everyKind.sort());
  await page.getByTestId("nav-next").click();
  expect(await nextKeys(page)).toEqual(pass);
});

test("C11, B10: a placeholder's card offers break down and mark atomic, and no done until it is atomic", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "triage");
  await expect(card(page)).toHaveAttribute("data-node", "n_kickoff");
  await card(page).getByRole("button", { name: "Mark reached" }).click();
  await openActing(page, "browser", journey, "triage?kind=deliverable");
  await expect.poll(() => passOrder(page)).toContain("n_workload");
  while ((await card(page).getAttribute("data-node")) !== "n_workload") {
    await card(page).getByTestId("pass").click();
  }
  const acts = card(page).getByTestId("acts");
  await expect(acts).toHaveAttribute("data-acts", "breakdown atomic snooze");
  await acts.getByRole("button", { name: "Mark atomic" }).click();
  await expect(acts).toHaveAttribute("data-acts", /\bdone\b/);
});

test("C11, C9: with every decision skipped in bulk, the walkthrough shows what would unblock the next ones", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "list?flag=decisions_needed");
  await expect.poll(() => listKeys(page)).toHaveLength(UP_FRONT.length);
  await page.getByRole("button", { name: "Select all shown" }).click();
  await page.getByRole("button", { name: "Skip..." }).click();
  await page.getByLabel("Why skip them").fill("decided elsewhere");
  await page.getByRole("button", { name: `Apply to ${String(UP_FRONT.length)}` }).click();
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
  await page.getByTestId("nav-walkthrough").click();
  const comparison = page.locator('[data-testid="waiting-decision"][data-node="n_comparison_set"]');
  await expect(comparison.locator('[data-testid="unblocker"][data-node="n_plan"]')).toBeVisible();
});

test("C11: a walkthrough filtered to only mine says the open decisions are others', not that none can be made", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "browser", journey, "triage?mode=decisions&mine=1");
  await expect(page.getByTestId("triage-empty")).toHaveAttribute("data-status", "filtered");
  await expect(page.getByTestId("waiting-decisions")).toHaveCount(0);
});
