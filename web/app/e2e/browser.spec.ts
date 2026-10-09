// The shell on the in-browser host (ARCHITECTURE, Web UI: in-browser host): no server, the
// fixtures seeded on load, each journey derived in the derive worker, a patch applied by the
// page's own service.
import { expect, test } from "@playwright/test";

import { derivedRevision, fresh, goTo, open, openJourney, recentSaves, rename } from "./shell.ts";

test("the index lists every fixture's journey, and a journey's document is derived in the worker", async ({ page }) => {
  await open(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  for (const name of ["Two-week bake-off", "Hire a platform engineer", "Launch the reporting release"]) {
    await expect(page.getByRole("link", { name })).toBeVisible();
  }
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("journey-name")).toHaveText("Hire a platform engineer");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/next\/list$/);
  expect(await derivedRevision(page)).toBe(6);
  await goTo(page, "plan", "graph");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await expect(page.locator("[data-testid=node-card][data-here]").first()).toBeVisible();
  expect(page.workers().length).toBeGreaterThan(0);
});

// Every error event the window sees, including the browser's own reports (a ResizeObserver
// loop) that never reach Playwright's pageerror.
test("a patch applies through the page's service, the view follows, and the canvas redraws with no error", async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.assign(window, { seenErrors: seen });
    window.addEventListener("error", (event) => seen.push(event.message));
  });
  await openJourney(page, "browser", "j_launch");
  const before = await derivedRevision(page);
  await rename(page, "n_docs", fresh("Docs"));
  expect(await recentSaves(page)).toEqual([expect.stringContaining("Edited")]);
  await expect.poll(() => derivedRevision(page)).toBe(before + 1);
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  expect(await page.evaluate(() => (window as unknown as { seenErrors: string[] }).seenErrors)).toEqual([]);
});
