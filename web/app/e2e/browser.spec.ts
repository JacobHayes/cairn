// The shell on the in-browser host (ARCHITECTURE, Web UI: in-browser host), one smoke test of
// what only a real browser shows: no server, the fixtures seeded on load, the module loaded by
// its address and each journey derived in the derive worker, the router redirecting an old
// address, a patch applied by the page's own service with no error event, and a screen whose
// file cannot be fetched asking for a reload while the frame stays. The module's agreement with
// the server is web/wasm's Node tests; every other test here starts from this host working.
import { expect, test } from "@playwright/test";

import { derivedRevision, fresh, goTo, goWithin, open, openJourney, recentSaves, rename, syncChip, visit } from "./shell.ts";

// Every error event the window sees, including the browser's own reports (a ResizeObserver
// loop) that never reach Playwright's pageerror.
test("the in-browser host boots, redirects, derives in the worker, applies a patch, and survives a screen that cannot load", async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.assign(window, { seenErrors: seen });
    window.addEventListener("error", (event) => seen.push(event.message));
  });
  // The landing opens Mine, and an old canvas address lands on its page (the table of old
  // addresses is journeys/address.test.ts's).
  await visit(page, "browser", "/");
  await expect(page).toHaveURL(/\/mine$/);
  await expect(page.getByTestId("mine-journey").first()).toBeVisible();
  await visit(page, "browser", "/journeys/j_hiring?hide=action&heat=on");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/plan\/graph\?kind=group%2Cdecision%2Cdeliverable%2Cmilestone&lens=gravity$/);

  await open(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("journey-name")).toHaveText("Hire a platform engineer");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/next\/list$/);
  expect(await derivedRevision(page)).toBe(6);
  await goTo(page, "plan", "graph");
  await expect(page.locator("[data-testid=node-card][data-here]").first()).toBeVisible();
  expect(page.workers().length).toBeGreaterThan(0);

  await openJourney(page, "browser", "j_launch");
  const before = await derivedRevision(page);
  await rename(page, "n_docs", fresh("Docs"));
  expect(await recentSaves(page)).toEqual([expect.stringContaining("Edited")]);
  await expect.poll(() => derivedRevision(page)).toBe(before + 1);
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  expect(await page.evaluate(() => (window as unknown as { seenErrors: string[] }).seenErrors)).toEqual([]);

  // A tab kept open across a deployment: a screen's file is gone, the frame stays, another screen opens.
  await page.route(/\/(src\/people|assets)\/Entities[.-]/, (route) => route.abort());
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Entities" }).click();
  await expect(page.getByTestId("load-failure").getByRole("button", { name: "Reload" })).toBeVisible();
  await expect(syncChip(page)).toBeVisible();
  await goWithin(page, "/journeys");
  await expect(page.getByTestId("load-failure")).toHaveCount(0);
});
