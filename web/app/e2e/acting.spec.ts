// The acting surfaces in Chromium, the two write paths no unit test reaches: the
// list's bulk bar (selection, one patch, the frontier rows and the folds under them, C9, B6) and the cards'
// pass (an answer, what it unlocked, passing, the inspector reached from the rail, a node
// completed from the pass, C11), over a journey started from a route's form (B1). Every test
// runs on the in-browser host, whose fixtures are fresh on every load. The rules behind both
// (ranking, filters, guards, the pass's order) are the engine's and vitest's.
import { expect, test, type Locator } from "@playwright/test";

import { answerCard, card, frontierItem, openActing, passOrder, publishFollowUpRoute, revisionAfter, select } from "./acting.ts";
import { journeyName, startJourney } from "./around.ts";
import { expectRowsInsideCards } from "./canvas.ts";
import { derivedRevision, goTo, nodeCard, nodePanel, setDepth } from "./shell.ts";

test("C9, B6: a bulk snooze is one patch; the snoozed leave the frontier and an unsnooze returns them", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "list?frontier=1");
  const revision = await derivedRevision(page);
  await select(page, "n_docs");
  await select(page, "n_announcement");
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "2");
  // A plain click on a row's title opens that node alone; checking the other row again makes two.
  await page.locator('[data-testid="list-row"][data-node="n_announcement"] a[data-node="n_announcement"]').click();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await select(page, "n_docs");
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "2");
  await page.getByTestId("selection-snooze").click();
  await page.getByRole("menuitem", { name: "Snooze until a node…" }).click();
  await page.getByLabel("Snooze them until node").selectOption("n_beta_end");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await expect(frontierItem(page, "n_beta_end")).toBeVisible();
  await expect(frontierItem(page, "n_docs")).toHaveCount(0);
  await expect(frontierItem(page, "n_announcement")).toHaveCount(0);
  const fold = page.getByTestId("fold-snoozed");
  await expect(fold).toHaveAttribute("data-count", "2");
  await fold.locator("summary").click();
  await expect(fold.getByTestId("snooze-group")).toHaveCount(1);
  // Snoozed is dimmed the same way on the graph, the list and the timeline (not now: lighter than resting, lighter still when done), its chip and date left clear, and what is not snoozed is not dimmed.
  // Frontier, on from the list, fades every card off the frontier, so it is off while the states themselves are read.
  await goTo(page, "graph");
  await page.keyboard.press("Shift+F");
  const opacityOf = (target: Locator) => target.evaluate((element) => Number(getComputedStyle(element).opacity));
  const snoozed = nodeCard(page, "n_docs");
  const onCard = await opacityOf(snoozed.locator(".node-title"));
  expect(onCard).toBeLessThan(1);
  expect(onCard).toBeGreaterThan(0.5);
  expect(await opacityOf(snoozed.getByTestId("card-state"))).toBe(1);
  expect(await opacityOf(snoozed.getByTestId("card-foot"))).toBe(1);
  await expect(snoozed.getByTestId("card-foot")).toHaveText(/^Until /);
  expect(await opacityOf(nodeCard(page, "n_beta_end").locator(".node-title"))).toBe(1);
  await expectRowsInsideCards(page);
  await goTo(page, "list");
  const row = page.locator('[data-testid="list-row"][data-node="n_docs"]');
  expect(await opacityOf(row.locator('[data-column="title"] .list-content'))).toBe(onCard);
  expect(await opacityOf(row.locator('[data-column="status"] .list-content'))).toBe(1);
  expect(await opacityOf(page.locator('[data-testid="list-row"][data-node="n_beta_end"] [data-column="title"] .list-content'))).toBe(1);
  await goTo(page, "timeline");
  const label = page.locator('[data-testid="timeline-entry"][data-node="n_docs"] .timeline-text');
  expect(await opacityOf(label.locator("> :not(.timeline-words)"))).toBe(onCard);
  expect(await opacityOf(label.getByTestId("timeline-words"))).toBe(1);
  await goTo(page, "list");
  await page.keyboard.press("Shift+F");
  await expect(fold).toHaveAttribute("data-count", "2");
  if ((await fold.getAttribute("open")) === null) {
    await fold.locator("summary").click();
  }
  await fold.locator('[data-testid="fold-item"][data-node="n_docs"]').getByRole("button", { name: "Unsnooze" }).click();
  await expect(frontierItem(page, "n_docs")).toBeVisible();
});

test("C11: the walkthrough's card sits in the middle of the canvas area at every width and has no way back to the graph", async ({ page }) => {
  for (const width of [1440, 900, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await openActing(page, "browser", "j_launch", "walk");
    await card(page).waitFor();
    const area = await page.locator(".journey-scroll").boundingBox();
    const frame = await card(page).boundingBox();
    if (area === null || frame === null) {
      throw new Error("the canvas area and the card are both drawn");
    }
    const [before, after] = [frame.x - area.x, area.x + area.width - (frame.x + frame.width)];
    expect(Math.abs(before - after), `the gaps either side of the card at ${String(width)}px`).toBeLessThanOrEqual(1);
    expect(frame.width).toBeLessThanOrEqual(560);
    expect(frame.x, "the card keeps the standard gutter from the window's edge").toBeGreaterThanOrEqual(16);
  }
  await expect(page.getByRole("link", { name: /back to graph/i })).toHaveCount(0);
});

test("B1, C11: a journey started from a route opens its walkthrough; an answer surfaces and unlocks work, passing writes nothing, and a node completed from the pass unlocks into it", async ({ page }) => {
  const route = await publishFollowUpRoute(page);
  await startJourney(page, "browser", journeyName("Follow-up"), { route, version: 1 });
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await expect(card(page)).toHaveAttribute("data-kind", "decision");
  await answerCard(page, "yes");
  await expect(page.getByTestId("surfaced").locator('[data-node="n_criteria"]')).toBeVisible();
  const answered = await derivedRevision(page);
  // The follow-up ranks below the up-front decisions still open, and still comes first.
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_scope");
  await expect(page.getByTestId("unlocked-by").getByRole("link", { name: "Partner runs testing" })).toBeVisible();
  await card(page).getByTestId("pass").click();
  await expect(page.getByTestId("unlocked-by")).toHaveCount(0);
  expect((await passOrder(page)).at(-1)).toBe("n_partner_scope");
  expect(await derivedRevision(page)).toBe(answered);
  // Every kind (Actions depth): the partner decision's other unlock is next, until a new pass returns to rank order.
  await setDepth(page, "Actions");
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

test("C9: a folded container stays folded across Show not relevant; a trip to the graph and back opens every level at Actions", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "list");
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", /\d/);
  const total = Number(await page.getByTestId("list-total").getAttribute("data-total"));
  // At Actions every level is open; the viewer folds one.
  await expect(page.locator('[data-testid="list-fold"][aria-expanded="false"]')).toHaveCount(0);
  const fold = page.getByTestId("list-fold").first();
  const folded = await fold.locator("xpath=ancestor::tr").getAttribute("data-node");
  await fold.click();
  const row = page.locator(`[data-testid="list-row"][data-node="${folded ?? ""}"] [data-testid="list-fold"]`);
  await expect(row).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByTestId("filter-badge")).toBeVisible();
  await page.getByTestId("filter-button").click();
  await page.getByTestId("show-not-relevant").click();
  await expect(page.getByTestId("list-total")).not.toHaveAttribute("data-total", String(total));
  await expect(row).toHaveAttribute("aria-expanded", "false");
  await page.getByTestId("show-not-relevant").click();
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", String(total));
  await goTo(page, "graph");
  await goTo(page, "list");
  await expect(page.getByTestId("list-fold").first()).toBeVisible();
  await expect(page.locator('[data-testid="list-fold"][aria-expanded="false"]')).toHaveCount(0);
});
