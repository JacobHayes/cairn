// The proof's media for brief 5.3 (briefs/proof/5.3/prove.sh): a screenshot of each
// acceptance state of the list, the next list, triage, and the decision walkthrough, a short
// video of the walkthrough, and the next list's values, written to CAIRN_PROOF_OUT.
// Steps only wait for the state they picture; the e2e specs assert it. The walkthrough runs on the server host over a journey started from the vendor
// evaluation's route; everything else on the in-browser host, seeded on each load.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Browser, type Page } from "@playwright/test";

import { answerCard, card, daysAfter, listKeys, nextItem, nextKeys, openActing, passOrder, revisionAfter, select, startVendorJourney } from "../e2e/acting.ts";
import { derivedRevision, goTo, syncChip } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(700);
const values: Record<string, unknown> = {};
const record = (name: string, value: unknown) => {
  values[name] = value;
  writeFileSync(join(out, "acting.json"), JSON.stringify(values, null, 2));
};

test.use({ viewport: { width: 1200, height: 900 } });
test.describe.configure({ mode: "serial" });

/** The next list's items with what each shows of its rank and slack. */
async function nextRows(page: Page): Promise<{ key: string; rank: string; slack: string }[]> {
  await nextKeys(page);
  return page.getByTestId("next-item").evaluateAll((items) =>
    items.map((item) => ({
      key: item.getAttribute("data-node") ?? "",
      rank: item.querySelector("[data-testid=why]")?.getAttribute("data-rank") ?? "",
      slack: item.getAttribute("data-slack") ?? "",
    })),
  );
}

test("the walkthrough at a journey's start, and what an answer surfaces", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "server", journey, "next/cards?decisions=1");
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  record("walkthroughStart", await passOrder(page));
  await shot(page, "1-walkthrough-at-the-start");
  await answerCard(page, "yes");
  await expect(page.getByTestId("surfaced").locator('[data-node="n_criteria"]')).toBeVisible();
  record("surfacedByPartner", await page.getByTestId("surfaced").locator("[data-node]").evaluateAll((links) => links.map((link) => link.getAttribute("data-node"))));
  await shot(page, "2-the-answer-surfaces-partner-work");
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => passOrder(page)).toContain("n_criteria");
  record("triageAfterPartner", await passOrder(page));
  await shot(page, "3-triage-every-kind");
});

test("a placeholder's card, and what would unblock the next decisions", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openActing(page, "server", journey, "next/cards");
  await card(page).getByRole("button", { name: "Mark reached" }).click();
  await openActing(page, "server", journey, "next/cards?kind=deliverable");
  await expect.poll(() => passOrder(page)).toContain("n_workload");
  while ((await card(page).getAttribute("data-node")) !== "n_workload") {
    await card(page).getByTestId("pass").click();
  }
  await expect(card(page).getByTestId("acts")).toHaveAttribute("data-acts", "atomic snooze");
  await shot(page, "4-placeholder-card");
  await openActing(page, "server", journey, "plan/list?flag=decisions_needed");
  await expect.poll(() => listKeys(page)).toHaveLength(5);
  await page.getByRole("button", { name: "Select all shown" }).click();
  await page.getByRole("button", { name: "Skip..." }).click();
  await page.getByLabel("Why skip them").fill("decided elsewhere");
  await page.getByRole("button", { name: "Apply to 5" }).click();
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
  await goTo(page, "next", "cards");
  await page.getByTestId("chip-decisions").click();
  await expect(page.getByTestId("waiting-decisions")).toBeVisible();
  record("waiting", await page.getByTestId("waiting-decision").evaluateAll((rows) =>
    rows.map((row) => ({ decision: row.getAttribute("data-node"), unblockers: [...row.querySelectorAll("[data-testid=unblocker]")].map((each) => each.getAttribute("data-node")) })),
  ));
  await shot(page, "5-what-would-unblock-the-next-decisions");
});

test("the next list ranked, re-sorted, and stalled", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "next/list");
  record("nextRanked", await nextRows(page));
  await shot(page, "6-next-ranked-with-why");
  await page.getByLabel("Sort by").selectOption("slack");
  await expect(page).toHaveURL(/sort=slack/);
  record("nextBySlack", await nextRows(page));
  await shot(page, "7-next-re-sorted-by-slack");
  await openActing(page, "browser", "j_hiring", "next/list");
  const today = (await syncChip(page).getAttribute("data-today")) ?? "";
  await nextItem(page, "n_offer").getByRole("button", { name: "Snooze until a date" }).click();
  await nextItem(page, "n_offer").getByLabel("Snooze until").fill(daysAfter(today, 7));
  await nextItem(page, "n_offer").getByTestId("snooze-date-form").getByRole("button", { name: "Save" }).click();
  await expect(page.locator('[data-testid="stall-cause"][data-status="snooze"]')).toBeVisible();
  await shot(page, "8-stalled-with-unsnooze");
});

test("the list: filters, grouping, search, bulk", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list?kind=deliverable,action&group=container");
  await expect(page.getByTestId("list-group").first()).toBeVisible();
  await shot(page, "9-list-filtered-and-grouped");
  await openActing(page, "browser", "j_vendor_eval", "plan/list?q=environment%20team");
  await expect.poll(() => listKeys(page)).toEqual(["n_access"]);
  await shot(page, "10-list-search-reads-notes");
  await openActing(page, "browser", "j_vendor_eval", "plan/list");
  const before = await derivedRevision(page);
  await select(page, "n_final_report");
  await select(page, "n_decision_meeting");
  await page.getByTestId("bulk-bar").getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.locator('[data-testid="violation"][data-node="n_final_report"]')).toBeVisible();
  record("bulkRejected", { before, after: await derivedRevision(page) });
  await shot(page, "11-bulk-done-rejected-naming-the-node");
  await openActing(page, "browser", "j_launch", "plan/list?flag=next_up");
  const snoozedFrom = await derivedRevision(page);
  await select(page, "n_docs");
  await select(page, "n_announcement");
  await page.getByRole("button", { name: "Snooze until a node..." }).click();
  await page.getByLabel("Snooze them until node").selectOption("n_beta_end");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  record("bulkSnooze", { before: snoozedFrom, after: await revisionAfter(page, snoozedFrom) });
  await openActing(page, "browser", "j_launch", "plan/list?flag=snoozed");
  await expect.poll(() => listKeys(page)).toHaveLength(2);
  await shot(page, "12-bulk-snoozed-in-one-patch");
});

/** The main flow: a fresh journey's walkthrough, an answer surfacing work, passes, triage, the next list. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 1100, height: 680 },
    recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 680 } },
  });
  const page = await context.newPage();
  const journey = await startVendorJourney(page);
  await openActing(page, "server", journey, "next/cards?decisions=1");
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await beat(page);
  await answerCard(page, "yes");
  await expect(page.getByTestId("surfaced")).toBeVisible();
  await beat(page);
  await page.keyboard.press("p");
  await beat(page);
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => passOrder(page)).toContain("n_criteria");
  await beat(page);
  await card(page).getByTestId("pass").click();
  await beat(page);
  await goTo(page, "next", "list");
  await beat(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
