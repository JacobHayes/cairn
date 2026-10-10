// The acting surfaces in Chromium (rung 6), the two write paths no unit test reaches: the
// list's bulk bar (selection, one patch, the next list and its folds, C9, B6) and the cards'
// pass (an answer, what it unlocked, passing, the inspector reached from the rail, a node
// completed from the pass, C11), over a journey started from a route's form (B1). Every test
// runs on the in-browser host, whose fixtures are fresh on every load. The rules behind both
// (ranking, filters, guards, the pass's order) are the engine's and vitest's.
import { expect, test } from "@playwright/test";

import { answerCard, card, nextItem, openActing, passOrder, publishFollowUpRoute, revisionAfter, select } from "./acting.ts";
import { journeyName, startJourney } from "./around.ts";
import { derivedRevision, goTo, nodePanel } from "./shell.ts";

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
