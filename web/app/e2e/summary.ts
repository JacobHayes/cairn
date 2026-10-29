// What the Summary's browser tests share: a fresh journey from a route, a journey
// stalled by snoozing what could be acted on, and reading the page's lists.
import { expect, type Locator, type Page } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { select } from "./acting.ts";
import { goTo, goWithin, openAt, whenSettled } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

/** A list panel of the Summary (`summary-next`, `summary-yours`, ...). */
export function panelOf(page: Page, testId: string): Locator {
  return page.getByTestId(testId);
}

/** How many rows the Summary list `testId` holds, drawn or not: its wrapper carries the count. */
export async function countOf(page: Page, testId: string): Promise<number> {
  return Number(await whenSettled(page, () => panelOf(page, testId).locator("xpath=ancestor-or-self::*[@data-count][1]").getAttribute("data-count")));
}

/** The rows of the Summary list `testId`, in the order they are drawn, drawn or not. */
export function rowsOf(page: Page, testId: string): Locator {
  return panelOf(page, testId).getByTestId("summary-item");
}

/** The keys of the rows `locator` finds. */
export function keysOf(locator: Locator): Promise<string[]> {
  return whenSettled(locator.page(), () => locator.evaluateAll((rows) => rows.map((row) => row.getAttribute("data-node") ?? "")));
}

/** Starts a journey from `route`'s first version (or with no node, with none) on cairn demo and opens its Summary. */
export async function startOnSummary(page: Page, route: string | undefined, name = journeyName("Summary")): Promise<void> {
  const id = await startJourney(page, name, route === undefined ? undefined : { route });
  // The journey's bare Summary, with the view state of the walkthrough it opened on left behind.
  await goWithin(page, `/journeys/${id}`);
  await expect(page.getByTestId("summary")).toBeVisible();
}

/** Snoozes every node `nodes` on journey `journey` until after the last due date, so nothing can be acted on, and opens its Summary. */
export async function stall(page: Page, journey: string, nodes: string[]): Promise<void> {
  await openAt(page, journey, "list?frontier=1", { fixedToday: FIXED_TODAY });
  for (const node of nodes) {
    await select(page, node);
  }
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", String(nodes.length));
  await page.getByTestId("selection-snooze").click();
  await page.getByRole("menuitem", { name: "Snooze…" }).click();
  await page.getByRole("radio", { name: "A date" }).check();
  await page.getByLabel("Snooze until", { exact: true }).fill("2026-12-31");
  await page.getByRole("button", { name: `Apply to ${String(nodes.length)}` }).click();
  await goTo(page, "summary");
  await expect(page.getByTestId("stalled")).toBeVisible();
}
