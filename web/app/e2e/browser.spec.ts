// The shell on the in-browser host (ARCHITECTURE, Web UI: in-browser host), one smoke test of
// what only a real browser shows: no server, the fixtures seeded on load, the module loaded by
// its address and each journey derived in the derive worker, the router (no redirects), a patch applied by the page's own service with no error event, and a screen whose
// file cannot be fetched asking for a reload while the frame stays. The module's agreement with
// the server is web/wasm's Node tests; every other test here starts from this host working.
// The timed parts of the shell that only a mounted page shows are here too: the sync dot's
// popover (peek, pin, the leave delay, Esc, a press outside, a resize), its OFFLINE state, and
// the toast held by the pointer.
import type { BrowserContext, Page } from "@playwright/test";

import { checkpoint, expect, test } from "./test.ts";

import { openRoute, routeMenuItem, savedText } from "./around.ts";
import { TOAST_MS } from "../src/data/toasts.ts";
import { SYNC_POPOVER_LEAVE_MS } from "../src/shell/sync-popover.ts";
import { derivedRevision, expectAddress, fresh, goTo, goWithin, openJourney, recentSaves, rename, syncChip, visit, whenSettled } from "./shell.ts";

const rail = (page: Page) => page.getByRole("navigation", { name: "Screens", exact: true });
const current = (page: Page) => rail(page).locator("[aria-current=page]");

/** Notes every error event the window sees (the browser's own reports, a ResizeObserver loop, never reach Playwright's pageerror) and every screen the shell paints, with the address it paints it at, from the first mutation on. */
async function watchWindow(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.assign(window, { seenErrors: seen });
    window.addEventListener("error", (event) => seen.push(event.message));
    const painted: string[] = [];
    Object.assign(window, { painted });
    const note = () => {
      const main = document.querySelector("main[data-screen]");
      const entry = main === null ? null : `${main.getAttribute("data-screen") ?? ""} @ ${location.pathname}${location.search} # ${main.querySelector("h1")?.textContent ?? ""}`;
      if (entry !== null && painted[painted.length - 1] !== entry) {
        painted.push(entry);
      }
    };
    new MutationObserver(note).observe(document, { subtree: true, childList: true, attributes: true });
  });
}

/** Opening `/` shows Home first and the page never moves after. */
async function opensOnHome(page: Page): Promise<void> {
  await visit(page, "browser", "/");
  await expect(page.getByTestId("home")).toBeVisible();
  await expect(page.getByTestId("home-journey").first()).toBeVisible();
  await expect(current(page)).toHaveText(["Home"]);
  // Once the host has settled, past the point where a data-driven redirect would have moved the page.
  await expect(syncChip(page)).toHaveAttribute("data-state", "demo");
  const painted = await whenSettled(page, () => page.evaluate(() => (window as unknown as { painted: string[] }).painted));
  const address = new URL(page.url()).pathname + new URL(page.url()).search;
  // Home's heading is in the first paint of the page, at the address it was opened at, and nothing after changes either.
  expect(painted).toEqual([`${address} @ ${address} # Waiting on you`]);
  expect(address).toBe("/");
  await checkpoint(page);
}

/** A tab kept open across a deployment: a screen's file is gone, the frame stays, another screen opens. */
async function survivesMissingScreen(page: Page): Promise<void> {
  await page.route(/\/(src\/people|assets)\/Identities[.-]/, (route) => route.abort());
  await rail(page).getByRole("link", { name: "Identities" }).click();
  await expect(page.getByTestId("load-failure").getByRole("button", { name: "Reload" })).toBeVisible();
  await expect(current(page)).toHaveText(["Identities"]);
  await expect(syncChip(page)).toBeVisible();
  await goWithin(page, "/");
  await expect(page.getByTestId("load-failure")).toHaveCount(0);
}

/** The dot says OFFLINE while the browser is offline, and DEMO, the in-browser host's resting state, when the network is back. */
async function saysOffline(page: Page, context: BrowserContext): Promise<void> {
  // A save just before shows SAVED for a moment; start from the resting state so the return to it is what is checked.
  await expect(syncChip(page)).toHaveAttribute("data-state", "demo");
  await context.setOffline(true);
  await expect(syncChip(page)).toHaveAttribute("data-state", "offline");
  await context.setOffline(false);
  await expect(syncChip(page)).toHaveAttribute("data-state", "demo");
  await checkpoint(page);
}

test("the in-browser host boots, derives in the worker, applies a patch, and survives a screen that cannot load; the rail marks the screen it is on, and the dot says OFFLINE", async ({ page, context }) => {
  await watchWindow(page);
  await opensOnHome(page);
  await visit(page, "browser", "/journeys");
  await expect(current(page)).toHaveText(["Journeys"]);

  await expect(page.getByTestId("journey-row")).toHaveCount(4);
  await page.getByRole("link", { name: "Hire a platform engineer" }).click();
  await expect(page.getByTestId("journey-name")).toHaveText("Hire a platform engineer");
  await expectAddress(page, "/journeys/j_hiring");
  await expect(current(page)).toHaveText(["Journeys"]);
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "summary");
  expect(await derivedRevision(page)).toBe(6);
  await goTo(page, "graph");
  await expect(page.locator("[data-testid=node-card][data-here]").first()).toBeVisible();
  expect(page.workers().length).toBeGreaterThan(0);

  await openJourney(page, "browser", "j_launch");
  const before = await derivedRevision(page);
  await rename(page, "n_docs", fresh("Docs"));
  expect(await recentSaves(page)).toEqual([expect.stringContaining("Edited")]);
  await expect.poll(() => derivedRevision(page)).toBe(before + 1);
  expect(await whenSettled(page, () => page.evaluate(() => (window as unknown as { seenErrors: string[] }).seenErrors))).toEqual([]);

  await rail(page).getByRole("link", { name: "Routes" }).click();
  await expect(current(page)).toHaveText(["Routes"]);
  expect(new URL(page.url()).pathname).toBe("/routes");
  await checkpoint(page);
  await survivesMissingScreen(page);
  await saysOffline(page, context);
});

test("the sync dot shows its popover on hover and focus, inside the window; it outlasts the pointer crossing the gap into it, and a pinned one closes on Esc (focus back on the dot), a press outside or a resize", async ({ page }) => {
  // The page's timers are the test's to run (the leave delay is a timer), so a slow machine cannot stretch the beat between two moves.
  await page.clock.install();
  await visit(page, "browser", "/");
  const chip = syncChip(page);
  const popover = page.getByTestId("sync-popover");
  await chip.hover();
  await expect(popover).toBeVisible();
  // Inside the window, and shown again from the keyboard's focus.
  const inside = await whenSettled(page, () => popover.boundingBox());
  const viewport = page.viewportSize();
  expect(inside !== null && viewport !== null && inside.x >= 0 && inside.x + inside.width <= viewport.width).toBe(true);
  await page.keyboard.press("Escape");
  await expect(popover).toHaveCount(0);
  await page.mouse.move(700, 500);
  await chip.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(popover).toBeVisible();
  await checkpoint(page);
  await page.keyboard.press("Escape");
  await expect(popover).toHaveCount(0);
  await chip.hover();
  await expect(popover).toBeVisible();
  await page.clock.pauseAt(new Date(Date.now() + 60_000));
  const [dot, sheet] = await whenSettled(page, () => Promise.all([chip.boundingBox(), popover.boundingBox()]));
  if (dot === null || sheet === null) {
    throw new Error("the dot and its popover have boxes");
  }
  // The pointer leaves the dot and rests in the gap, then reaches the popover before the leave delay is up, and the popover stays past the first leave's deadline.
  await page.mouse.move(dot.x + dot.width / 2, (dot.y + dot.height + sheet.y) / 2);
  await page.clock.runFor(SYNC_POPOVER_LEAVE_MS / 2);
  await page.mouse.move(sheet.x + sheet.width / 2, sheet.y + 20);
  await page.clock.runFor(2 * SYNC_POPOVER_LEAVE_MS);
  await expect(popover).toBeVisible();
  await page.clock.resume();
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

test("a toast stays while the pointer is on it, the next one replaces it, and it goes after the pointer leaves", async ({ page }) => {
  await openRoute(page, "browser", "hiring-loop");
  // The toast's time is a timer the test runs, so a slow machine cannot let it lapse before the pointer is on it.
  await page.clock.install();
  await page.clock.pauseAt(new Date(Date.now() + 60_000));
  const toast = page.getByTestId("toast");
  await savedText(page, () => routeMenuItem(page, "menu-export"));
  await expect(toast).toHaveText("Route file exported");
  // Whether the pointer is on the toast, as the browser has it: the time a pointer move has been taken in, since the toast's hold follows the browser's enter and leave.
  const pointerOn = (on: boolean) => expect.poll(() => toast.evaluate((element) => element.matches(":hover"))).toBe(on);
  await toast.hover();
  await pointerOn(true);
  // Past the toast's time, the pointer holds it.
  await page.clock.runFor(TOAST_MS.confirm + 1000);
  await expect(toast).toBeVisible();
  // The next toast, raised from the keyboard with the pointer still on the toast, replaces it (a new element) and is held too.
  const first = await toast.elementHandle();
  await page.getByTestId("route-menu").focus();
  await page.keyboard.press("Enter");
  await page.getByTestId("menu-export").focus();
  await savedText(page, () => page.keyboard.press("Enter"));
  await expect.poll(() => first.evaluate((element) => element.isConnected)).toBe(false);
  await expect(toast).toHaveCount(1);
  await pointerOn(true);
  // A new element under a still pointer is entered at the browser's next pointer move, which a loaded machine may take late: move within the toast, so its enter has come before the pointer leaves.
  const box = (await whenSettled(page, () => toast.boundingBox())) ?? { x: 0, y: 0, width: 0, height: 0 };
  await page.mouse.move(box.x + box.width / 2 + 2, box.y + box.height / 2 + 2);
  await page.clock.runFor(TOAST_MS.confirm + 1000);
  await expect(toast).toBeVisible();
  // Once the pointer has left, the toast goes after its full time.
  await page.mouse.move(5, 5);
  await pointerOn(false);
  await page.clock.runFor(TOAST_MS.confirm + 1000);
  await expect(toast).toHaveCount(0);
});
