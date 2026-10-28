// The acting surfaces in Chromium, the two write paths no unit test reaches: the
// list's bulk bar (selection, one patch, the frontier rows and the folds under them, C9, B6) and the cards'
// pass (an answer, what it unlocked, passing, the inspector reached from the rail, a node
// completed from the pass, C11), over a journey started from a route's form (B1). Every test
// runs on the in-browser host, whose fixtures are fresh on every load. The rules behind both
// (ranking, filters, guards, the pass's order) are the engine's and vitest's.
import type { Locator } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { answerCard, card, frontierItem, openActing, passOrder, publishFollowUpRoute, revisionAfter, select } from "./acting.ts";
import { journeyName, startJourney } from "./around.ts";
import { expectStructure } from "./structure.ts";
import { derivedRevision, expectAddress, goTo, nodeCard, nodePanel, open, openAt, readable, setDepth, whenSettled } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

test("C9, B6: a bulk snooze is one patch; the snoozed leave the frontier and an unsnooze returns them", async ({ page }) => {
  await readable(page);
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
  await page.getByRole("menuitem", { name: "Snooze…" }).click();
  await page.getByRole("radio", { name: "When something is done" }).check();
  await page.getByLabel("Snooze until node").selectOption("n_beta_end");
  await page.getByRole("button", { name: "Apply to 2" }).click();
  expect(await revisionAfter(page, revision)).toBe(revision + 1);
  await expect(frontierItem(page, "n_beta_end")).toBeVisible();
  await expect(frontierItem(page, "n_docs")).toHaveCount(0);
  await expect(frontierItem(page, "n_announcement")).toHaveCount(0);
  const fold = page.getByTestId("fold-snoozed");
  await expect(fold).toHaveAttribute("data-count", "2");
  await fold.locator("summary").click();
  await expect(fold.getByTestId("snooze-group")).toHaveCount(1);
  // Snoozed is dimmed the same way on the graph and the list (not now: lighter than resting, lighter still when done), its chip and date left clear, and what is not snoozed is not dimmed.
  // Frontier, on from the list, fades every card off the frontier, so it is off while the states themselves are read.
  await goTo(page, "graph");
  await page.keyboard.press("Shift+F");
  const opacityOf = (target: Locator) => whenSettled(page, () => target.evaluate((element) => Number(getComputedStyle(element).opacity)));
  const snoozed = nodeCard(page, "n_docs");
  const onCard = await opacityOf(snoozed.locator(".node-title"));
  expect(onCard).toBeLessThan(1);
  expect(onCard).toBeGreaterThan(0.5);
  expect(await opacityOf(snoozed.getByTestId("card-state"))).toBe(1);
  expect(await opacityOf(snoozed.getByTestId("card-foot"))).toBe(1);
  await expect(snoozed.getByTestId("card-foot")).toHaveText(/^Until /);
  expect(await opacityOf(nodeCard(page, "n_beta_end").locator(".node-title"))).toBe(1);
  await expectStructure(page);
  const footWords = await whenSettled(page, () => snoozed.getByTestId("card-foot").innerText());
  await goTo(page, "list");
  const row = page.locator('[data-testid="list-row"][data-node="n_docs"]');
  // Under its title the row says where the snooze ends, in the words of the card's foot; an unsnoozed row says nothing.
  await expect(row.getByTestId("list-return")).toHaveText(footWords);
  expect(await whenSettled(page, () => row.getByTestId("list-return").evaluate((element) => Number(getComputedStyle(element.closest(".list-content") ?? element).opacity)))).toBe(1);
  await expect(page.locator('[data-testid="list-row"][data-node="n_beta_end"]').getByTestId("list-return")).toHaveCount(0);
  expect(await opacityOf(row.locator('[data-column="title"] .list-node > a'))).toBe(onCard);
  expect(await opacityOf(row.locator('[data-column="status"] .list-content'))).toBe(1);
  expect(await opacityOf(page.locator('[data-testid="list-row"][data-node="n_beta_end"] [data-column="title"] .list-content'))).toBe(1);
  await goTo(page, "list");
  await page.keyboard.press("Shift+F");
  await expect(fold).toHaveAttribute("data-count", "2");
  if ((await whenSettled(page, () => fold.getAttribute("open"))) === null) {
    await fold.locator("summary").click();
  }
  await fold.locator('[data-testid="fold-item"][data-node="n_docs"]').getByRole("button", { name: "Unsnooze" }).click();
  await expect(frontierItem(page, "n_docs")).toBeVisible();
});

test("C11: the walkthrough's card sits in the middle of the canvas area at every width, and is the node's one editor at the address it was opened at", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "walk");
  await card(page).waitFor();
  for (const width of [1440, 900, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(async () => {
      const area = await page.locator(".journey-scroll").boundingBox();
      const frame = await card(page).boundingBox();
      if (area === null || frame === null) {
        throw new Error("the canvas area and the card are both drawn");
      }
      const [before, after] = [frame.x - area.x, area.x + area.width - (frame.x + frame.width)];
      expect(Math.abs(before - after), `the gaps either side of the card at ${String(width)}px`).toBeLessThanOrEqual(1);
      expect(frame.width).toBeLessThanOrEqual(560);
      expect(frame.x, "the card keeps the standard gutter from the window's edge").toBeGreaterThanOrEqual(16);
    }).toPass();
  }
  await checkpoint(page);
  // Opened at a node's own address, the card is that node's one editor; the inspector does not open a second one.
  const first = ((await whenSettled(page, () => page.getByTestId("triage").getAttribute("data-order"))) ?? "").split(" ")[0] ?? "";
  const at = `/journeys/j_launch/nodes/${first}?view=walk`;
  await open(page, "browser", at);
  await expect(page.getByTestId("triage")).toBeVisible();
  await expectAddress(page, at);
  await expect(page.getByTestId("node-detail")).toHaveCount(1);
  await expect(page.getByTestId("triage").getByTestId("node-detail")).toHaveAttribute("data-node", first);
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
  await checkpoint(page);
  // With every decision made (the seeded vendor evaluation) there is nothing to decide, and Continue goes on to everything.
  await openAt(page, "browser", "j_vendor_eval", "walk?depth=decisions", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("no-decisions")).toBeVisible();
  await page.getByRole("button", { name: "Continue with everything" }).click();
  await expectAddress(page, "/journeys/j_vendor_eval?view=walk");
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Actions");
});
