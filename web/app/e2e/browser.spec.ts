// The shell on the in-browser host (ARCHITECTURE, Web UI: in-browser host), one smoke test of
// what only a real browser shows: no server, the fixtures seeded on load, the module loaded by
// its address and each journey derived in the derive worker, the router redirecting an old
// address, a patch applied by the page's own service with no error event, and a screen whose
// file cannot be fetched asking for a reload while the frame stays. The module's agreement with
// the server is web/wasm's Node tests; every other test here starts from this host working.
// The timed parts of the shell that only a mounted page shows are here too: the sync dot's
// popover (peek, pin, the leave delay, Esc, a press outside, a resize), its OFFLINE state, and
// the toast held by the pointer.
import { expect, test } from "@playwright/test";

import { newRoute, routeName } from "./authoring.ts";
import { savedText } from "./around.ts";
import { derivedRevision, fresh, goWithin, open, openJourney, recentSaves, rename, syncChip, visit } from "./shell.ts";

// Every error event the window sees, including the browser's own reports (a ResizeObserver
// loop) that never reach Playwright's pageerror.
test("the in-browser host boots, derives in the worker, applies a patch, and survives a screen that cannot load", async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.assign(window, { seenErrors: seen });
    window.addEventListener("error", (event) => seen.push(event.message));
  });
  await visit(page, "browser", "/");
  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  expect(new URL(page.url()).pathname + new URL(page.url()).search).toBe("/");

  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("journey-name")).toHaveText("Hire a platform engineer");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/graph$/);
  expect(await derivedRevision(page)).toBe(6);
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
  await goWithin(page, "/");
  await expect(page.getByTestId("load-failure")).toHaveCount(0);
});

test("opening `/` shows the journey index first and the page never moves after", async ({ page }) => {
  // Every screen the shell paints, and the address it paints it at, from the first mutation on.
  await page.addInitScript(() => {
    const painted: string[] = [];
    Object.assign(window, { painted });
    const note = () => {
      const main = document.querySelector("main[data-screen]");
      const entry = main === null ? null : `${main.getAttribute("data-screen") ?? ""} @ ${location.pathname}${location.search}`;
      if (entry !== null && painted[painted.length - 1] !== entry) {
        painted.push(entry);
      }
    };
    new MutationObserver(note).observe(document, { subtree: true, childList: true, attributes: true });
  });
  await visit(page, "browser", "/");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  // Past the point where a data-driven redirect would have moved the page.
  await page.waitForTimeout(1000);
  const painted = await page.evaluate(() => (window as unknown as { painted: string[] }).painted);
  const address = new URL(page.url()).pathname + new URL(page.url()).search;
  expect(painted).toEqual([`${address} @ ${address}`]);
  expect(address).toBe("/");
});

test("the rail marks the journey index current at `/` and only there", async ({ page }) => {
  await visit(page, "browser", "/");
  const rail = page.getByRole("navigation", { name: "Screens", exact: true });
  const current = rail.locator("[aria-current=page]");
  await expect(current).toHaveText(["Journeys"]);
  await rail.getByRole("link", { name: "Library" }).click();
  await expect(current).toHaveText(["Library"]);
  await rail.getByRole("link", { name: "Journeys" }).click();
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(current).toHaveText(["Journeys"]);
  expect(new URL(page.url()).pathname).toBe("/");
  await page.goto(`${new URL(page.url()).origin}/?status=any`);
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(current).toHaveText(["Journeys"]);
});

test("the sync dot shows its popover on hover and focus, inside the window, and a click pins it", async ({ page }) => {
  await visit(page, "browser", "/");
  const chip = syncChip(page);
  const popover = page.getByTestId("sync-popover");
  await chip.hover();
  await expect(popover).toBeVisible();
  const box = await popover.boundingBox();
  const size = page.viewportSize();
  expect(box !== null && size !== null && box.x >= 0 && box.x + box.width <= size.width).toBe(true);
  await page.keyboard.press("Escape");
  await expect(popover).toHaveCount(0);
  await page.mouse.move(700, 500);
  await chip.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(popover).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(popover).toHaveCount(0);
  await chip.click();
  await page.mouse.move(700, 500);
  await expect(popover).toBeVisible();
  await expect(chip).toHaveAttribute("aria-expanded", "true");
});

test("the sync dot's popover outlasts the pointer crossing the gap into it, and a pinned one closes on Esc (focus back on the dot), a press outside or a resize", async ({ page }) => {
  await visit(page, "browser", "/");
  const chip = syncChip(page);
  const popover = page.getByTestId("sync-popover");
  await chip.hover();
  await expect(popover).toBeVisible();
  // Once it has settled (it rises in), where it is.
  await page.waitForTimeout(400);
  const dot = await chip.boundingBox();
  const sheet = await popover.boundingBox();
  if (dot === null || sheet === null) {
    throw new Error("the dot and its popover have boxes");
  }
  // The pointer leaves the dot and rests in the gap, then reaches the popover before the leave delay (150ms) is up, and the popover stays past the first leave's deadline.
  await page.mouse.move(dot.x + dot.width / 2, (dot.y + dot.height + sheet.y) / 2);
  await page.waitForTimeout(100);
  await page.mouse.move(sheet.x + sheet.width / 2, sheet.y + 20);
  await page.waitForTimeout(300);
  await expect(popover).toBeVisible();
  // Pinned, Esc closes it and the dot has the focus again.
  await chip.click();
  await page.keyboard.press("Escape");
  await expect(popover).toHaveCount(0);
  await expect(chip).toBeFocused();
  // Pinned, a press outside closes it.
  await chip.click();
  await page.mouse.move(700, 500);
  await expect(popover).toBeVisible();
  await page.mouse.click(700, 500);
  await expect(popover).toHaveCount(0);
  // Pinned, a resize closes it rather than leaving it adrift.
  await chip.click();
  await page.mouse.move(700, 500);
  await expect(popover).toBeVisible();
  const size = page.viewportSize();
  await page.setViewportSize({ width: (size?.width ?? 1280) - 40, height: size?.height ?? 720 });
  await expect(popover).toHaveCount(0);
});

test("the sync dot says OFFLINE while the browser is offline, and the state it had when the network is back", async ({ page, context }) => {
  await visit(page, "browser", "/");
  const chip = syncChip(page);
  await expect(chip).toHaveAttribute("data-state", "demo");
  await context.setOffline(true);
  await expect(chip).toHaveAttribute("data-state", "offline");
  await context.setOffline(false);
  await expect(chip).toHaveAttribute("data-state", "demo");
});

test("a toast stays while the pointer is on it, the next one replaces it, and it goes after the pointer leaves", async ({ page }) => {
  const route = await newRoute(page, "browser", routeName("Toasting"));
  await open(page, "browser", `/routes/${route}/versions`);
  const exportDraft = page.getByTestId("route-actions").getByRole("button", { name: "Export the draft" });
  const toast = page.getByTestId("toast");
  await savedText(page, () => exportDraft.click());
  await expect(toast).toHaveText("Route file exported");
  await toast.hover();
  // Past the toast's time, the pointer holds it.
  await page.waitForTimeout(3500);
  await expect(toast).toBeVisible();
  // The next toast, raised from the keyboard with the pointer still on the toast, replaces it and is held too.
  await exportDraft.focus();
  await savedText(page, () => page.keyboard.press("Enter"));
  await expect(toast).toHaveCount(1);
  await page.waitForTimeout(3500);
  await expect(toast).toBeVisible();
  await page.mouse.move(5, 5);
  await expect(toast).toHaveCount(0);
});
