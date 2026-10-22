// PRD: C20. A guided first run on the in-browser host: the welcome opens on Home
// and never over a deep link; a practice journey is made on Start, walked through the ordinary
// screens, and deleted when the tour ends. What each step says is composed from the state the
// engine derived (steps.test.ts); here the oracle is the screen: which rows became ready, which
// card became conditional, where the person is.
import type { Page } from "@playwright/test";

import { checkpoint, expect, test } from "./test.ts";

import { setFrontier } from "./acting.ts";
import { nodePanel, open, openFromCanvas, visit, whenSettled } from "./shell.ts";

const band = (page: Page) => page.getByTestId("tour-band");

/** Starts the tour from Home and returns the practice journey's id. */
async function start(page: Page): Promise<string> {
  await open(page, "browser", "/");
  await page.getByTestId("tour-start").click();
  await expect(band(page)).toHaveAttribute("data-step", "map");
  const match = /\/journeys\/(j_[^/?]+)/.exec(page.url());
  expect(match).not.toBeNull();
  return match?.[1] ?? "";
}

/** Chooses `option` for the practice decision in its open inspector and saves. */
async function decide(page: Page, option: "yes" | "no"): Promise<void> {
  const panel = nodePanel(page, "n_outside");
  await panel.getByRole("radio", { name: new RegExp(`^${option}\\b`, "i") }).check();
  await panel.getByRole("button", { name: "Save", exact: true }).click();
}

async function next(page: Page): Promise<void> {
  const button = page.getByTestId("tour-next");
  await expect(button).not.toHaveAttribute("aria-disabled", "true");
  await button.click();
}

test("the welcome opens on Home and never over a deep link", async ({ page }) => {
  await visit(page, "browser", "/journeys");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  // The rail offers it quietly; the link opens the welcome on Home.
  await page.getByTestId("rail-tour").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
  await expect(page.getByTestId("rail-tour")).toHaveCount(0);
  await visit(page, "browser", "/");
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
  await checkpoint(page);
  // Not now ends the offers; the rail and You still offer the tour.
  await page.getByTestId("tour-not-now").click();
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  await page.reload();
  await expect(page.getByTestId("rail-tour")).toHaveCount(0);
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  await page.getByRole("link", { name: "You", exact: true }).click();
  await page.getByTestId("you-tour").click();
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
});

test("the tour walks a practice journey, and the journey is gone at the end", async ({ page }) => {
  await open(page, "browser", "/journeys?status=any");
  const journeyCount = await whenSettled(page, () => page.getByTestId("journey-row").count());
  const id = await start(page);
  // Step 1: nothing is selected; Next waits for the decision to be selected.
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await expect(band(page)).toContainText("Write the draft");
  await checkpoint(page);
  await openFromCanvas(page, "n_outside");
  await expect(page.getByTestId("tour-result")).not.toBeEmpty();
  await next(page);
  // Step 2: the answer decides whether the review applies.
  await expect(band(page)).toHaveAttribute("data-step", "answer");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await decide(page, "yes");
  await expect(page.getByTestId("tour-result")).toContainText("applies now");
  await next(page);
  // Step 3: Frontier.
  await expect(band(page)).toHaveAttribute("data-step", "frontier");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await setFrontier(page, true);
  await expect(page.getByTestId("tour-result")).toContainText("can move now");
  await next(page);
  // Step 4: finish the draft in the list; the work that waited for it is ready.
  await expect(band(page)).toHaveAttribute("data-step", "finish");
  await expect(page).toHaveURL(/view=list/);
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  // The due date is said once: the Due column has it, so the fact under the title does not repeat it.
  await expect(page.locator('tr[data-node="n_draft"] [data-testid="list-due"]')).toBeVisible();
  await expect(page.locator('tr[data-node="n_draft"] [data-testid="fact"]')).toBeHidden();
  await page.locator('tr[data-node="n_draft"]').getByRole("button", { name: /done/i }).click();
  await expect(page.getByTestId("tour-result")).toContainText("ready now");
  await checkpoint(page);
  // Going back keeps what was done; the earlier step says what is true now.
  await page.getByTestId("tour-back").click();
  await expect(band(page)).toHaveAttribute("data-step", "frontier");
  await expect(page.getByTestId("tour-next")).not.toHaveAttribute("aria-disabled", "true");
  await next(page);
  await next(page);
  // Step 5: dates, then the end.
  await expect(band(page)).toHaveAttribute("data-step", "dates");
  await expect(page).toHaveURL(/view=timeline/);
  await checkpoint(page);
  await page.getByTestId("tour-next").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-end")).toBeVisible();
  await checkpoint(page);
  await page.getByTestId("tour-close").click();
  await expect(page.getByTestId("tour-end")).toHaveCount(0);
  // The practice journey is deleted and no offer comes back.
  await open(page, "browser", "/journeys?status=any");
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toHaveCount(0);
  await expect(page.getByTestId("journey-row")).toHaveCount(journeyCount);
  await open(page, "browser", "/");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
});

test("the answer No leaves one newly ready row", async ({ page }) => {
  await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  await decide(page, "no");
  await expect(page.getByTestId("tour-result")).toContainText("does not apply");
  await next(page);
  await setFrontier(page, true);
  await next(page);
  await page.locator('tr[data-node="n_draft"]').getByRole("button", { name: /done/i }).click();
  await expect(page.getByTestId("tour-result")).toContainText("ready now");
  await expect(page.getByTestId("tour-result")).not.toContainText("Outside review");
});

test("Skip returns Home and the practice journey is deleted", async ({ page }) => {
  const id = await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  await page.getByTestId("tour-skip").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-band")).toHaveCount(0);
  await expect(page.getByTestId("tour-end")).toHaveCount(0);
  await open(page, "browser", "/journeys?status=any");
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toHaveCount(0);
});

test("leaving the practice journey pauses the tour, and Continue returns to the step", async ({ page }) => {
  const id = await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  await page.getByRole("link", { name: "Journeys", exact: true }).click();
  await expect(page.getByTestId("tour-paused")).toBeVisible();
  await expect(page.getByTestId("tour-band")).toHaveCount(0);
  await page.getByRole("link", { name: "Home", exact: true }).click();
  await expect(page.getByTestId("tour-continue")).toBeVisible();
  await page.getByTestId("tour-resume").click();
  await expect(page).toHaveURL(new RegExp(`/journeys/${id}`));
  await expect(band(page)).toHaveAttribute("data-step", "answer");
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test("the band is compact, the sheet is raised for the answer, and Frontier is reached through Controls", async ({ page }) => {
    await start(page);
    await checkpoint(page);
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await decide(page, "yes");
    // The pane is inert at the peek: the sheet lowers and the band takes the focus.
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "peek");
    await expect(page.getByTestId("tour-result")).toContainText("applies now");
    await expect(page.getByTestId("tour-instruction")).toHaveCount(0);
    await checkpoint(page);
    await next(page);
    await expect(page.getByTestId("controls-button")).toBeVisible();
    await setFrontier(page, true);
    await next(page);
    await page.locator('tr[data-node="n_draft"]').getByRole("button", { name: /done/i }).click();
    await expect(page.getByTestId("tour-result")).toContainText("ready now");
    await expect(page.getByTestId("tour-next")).toBeVisible();
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });
});
