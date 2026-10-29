// PRD: C20. The tour's record is the user's, so two tabs pressing Start together share one
// practice journey.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { open } from "./shell.ts";

test("two tabs pressing Start together share one practice journey", async ({ page, context }) => {
  await open(page, "/me");
  await page.getByTestId("you-tour").click();
  const other = await context.newPage();
  await open(other, "/me");
  await other.getByTestId("you-tour").click();
  await Promise.all([page.getByTestId("tour-start").click(), other.getByTestId("tour-start").click()]);
  const journeyOf = (tab: Page) => /\/journeys\/(j_[^/?]+)/.exec(tab.url())?.[1];
  // The winner is on its journey once the journey exists; the loser may be anywhere.
  await expect.poll(() => journeyOf(page) ?? journeyOf(other)).toBeDefined();
  // Whichever tab lost the race finds the winner's run waiting; both resume the one journey.
  for (const tab of [page, other]) {
    await tab.goto("/");
    await expect(tab.getByTestId("tour-continue")).toBeVisible();
    await tab.getByTestId("tour-resume").click();
    await expect.poll(() => journeyOf(tab)).toBeDefined();
  }
  expect(journeyOf(other)).toBe(journeyOf(page));
  await page.getByTestId("tour-skip").click();
  await expect(page).toHaveURL(/\/$/);
  await other.close();
});
