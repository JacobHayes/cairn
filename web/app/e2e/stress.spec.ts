// The containment invariant over worst-case content (e2e/stress.ts): the stress journey walked
// through every screen and width. Each step ends in a check (e2e/test.ts runs the
// invariant after the test; `look` runs it between the steps, so a failure names the
// screen).
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { expectTitleLinesAsLaidOut, viewportOf } from "./canvas.ts";
import { NEAR_ZOOM } from "../src/canvas/gestures.ts";
import { observe } from "./structure.ts";
import { indexAt, journeyAt, openCardMenu, openControlsMenu, openPopover, routeAt } from "./screenshots.ts";
import { nodeCard, settled, whenSettled } from "./shell.ts";
import { STRESS, publishStressJourney } from "./stress.ts";

/** The walk's reach: every screen is looked at and every problem listed with the screen it is on, so one run shows them all. */
interface Walk {
  page: Page;
  found: string[];
  look: (where: string) => Promise<void>;
  check: (address: string, node?: string) => Promise<void>;
}

function walkOf(page: Page, journey: string): Walk {
  const found: string[] = [];
  const look = async (where: string) => {
    found.push(...Object.values((await observe(page)).problems).flat().map((problem) => `${where}: ${problem}`));
  };
  const check = async (address: string, node?: string) => {
    await journeyAt(page, journey, address, node);
    await look(`${address}${node === undefined ? "" : ` with ${node}`} at ${String(page.viewportSize()?.width ?? 0)}px`);
  };
  return { page, found, look, check };
}

/** The graph fitted, at the reading zoom in a window that holds it (every card drawn at the near zoom), and with the stages folded. */
async function graphs({ page, look, check }: Walk): Promise<void> {
  await check("graph");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.keyboard.press("f");
  await settled(page);
  await look("graph fitted");
  await page.setViewportSize({ width: 4200, height: 3200 });
  await check("graph");
  await page.keyboard.press("f");
  await settled(page);
  expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(NEAR_ZOOM);
  await look("graph at the reading zoom");
  await expectTitleLinesAsLaidOut(page);
  // The foot's owner is a fact the card states, so it is never cut: the date beside it gives way.
  const owners = page.getByTestId("card-owner");
  const clipped = await whenSettled(page, () => owners.evaluateAll((all) => ({ all: all.length, clipped: all.filter((each) => each.scrollWidth > each.clientWidth).length })));
  expect(clipped.all).toBeGreaterThan(0);
  expect(clipped.clipped).toBe(0);
  // Folded, a stage is a card of the usual width: its head holds the kind, CURRENT, the chip and the control.
  for (const stage of STRESS.stages) {
    const fold = nodeCard(page, stage).getByTestId("card-collapse");
    if ((await whenSettled(page, () => fold.count())) > 0) {
      await fold.click();
    }
  }
  await page.keyboard.press("f");
  await settled(page);
  await look("graph with the stages folded");
  await page.setViewportSize({ width: 1280, height: 720 });
}

/** The list, timeline and summary; the inspector with every section open; the popovers and a card's menu, each holding its own content. */
async function details({ page, look, check }: Walk): Promise<void> {
  await check("list");
  await check("timeline");
  await check("summary");
  await check("list", "n_a1");
  await page.evaluate(() => { document.querySelectorAll<HTMLDetailsElement>(".inspector details").forEach((section) => { section.open = true; }); });
  await look("the inspector with every section open");
  for (const trigger of ["filter-button", "depth-button", "view-picker", "sync"] as const) {
    await openPopover(page, trigger);
    await look(`${trigger} open`);
    await page.keyboard.press("Escape");
  }
  await check("graph");
  await openCardMenu(page, "n_a1");
  await look("a card's menu");
  await page.keyboard.press("Escape");
  await check("walk");
}

/** The stress route's own graph, fitted and at the reading zoom in a window that holds it. */
async function routeGraph({ page, look }: Walk): Promise<void> {
  await routeAt(page, STRESS.route, "graph");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.keyboard.press("f");
  await settled(page);
  await look("the route's graph fitted");
  await page.setViewportSize({ width: 4200, height: 3200 });
  await routeAt(page, STRESS.route, "graph");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.keyboard.press("f");
  await settled(page);
  expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(NEAR_ZOOM);
  await look("the route's graph at the reading zoom");
  await expectTitleLinesAsLaidOut(page);
  await page.setViewportSize({ width: 1280, height: 720 });
}

/** A phone's chrome: the inspector hidden to its bar, the sync popover with its You link, and the stress route's list and timeline. */
async function phoneChrome({ page, look, check }: Walk): Promise<void> {
  await check("list", "n_a1");
  await page.getByRole("button", { name: "Hide inspector" }).click();
  await look("the inspector folded at 390px");
  await openPopover(page, "sync");
  await look("the sync popover at 390px");
  await page.keyboard.press("Escape");
  for (const view of ["list", "timeline"]) {
    await routeAt(page, STRESS.route, view);
    await look(`the route's ${view} at 390px`);
  }
}

/** Each screen at desktop and phone width (a phone also opens each view's Controls menu), the clamped journey name opened by a tap, and the screens that list journeys, routes and people. */
async function widths(walk: Walk, sizes: number[]): Promise<void> {
  const { page, look, check } = walk;
  for (const width of sizes) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 720 });
    for (const address of ["graph", "list", "timeline", "summary", "walk"]) {
      await check(address);
      if (width === 390 && address !== "summary") {
        await openControlsMenu(page);
        await look(`the Controls menu on ${address} at 390px`);
        await page.keyboard.press("Escape");
      }
    }
    const name = page.getByTestId("journey-name");
    const clamped = await whenSettled(page, () => name.getByRole("button").evaluate((toggle) => toggle.scrollHeight > toggle.clientHeight));
    expect(clamped || width > 390, "the stress name is long enough to be clamped on a phone").toBe(true);
    // A name cut by its clamp wears a mark, and a tap opens the name and leaves the mark.
    if (width === 390) {
      await expect(page.getByTestId("journey-name-cue")).toBeVisible();
    }
    await name.getByRole("button").click();
    await expect(name.getByRole("button")).toHaveAttribute("aria-expanded", "true");
    await look(`the journey name opened at ${String(width)}px`);
    if (width === 390) {
      await expect(page.getByTestId("journey-name-cue")).toBeVisible();
      await phoneChrome(walk);
    }
    for (const path of ["/", "/journeys", "/routes", "/identities", "/me"]) {
      await indexAt(page, path);
      await look(`${path} at ${String(width)}px`);
    }
  }
}

test("the stress journey stays inside its boxes on every screen at desktop width", async ({ page }) => {
  const walk = walkOf(page, await publishStressJourney(page));
  await graphs(walk);
  await routeGraph(walk);
  await details(walk);
  await widths(walk, [1280]);
  expect(walk.found).toEqual([]);
});

// A phone is a touch screen: the same walk there also holds every control to the touch-sized hit area.
test.describe("on a phone", () => {
  // The journey is published from a desktop-sized window, as the other walk does, then the walk resizes it to a phone's.
  test.use({ viewport: { width: 1280, height: 720 }, hasTouch: true });
  // Every look also measures every control against the touch target, which is slow on the worst-case journey.
  test.slow();

  test("the stress journey stays inside its boxes and its controls are touch-sized on every screen, and a whole name wears no clamp mark", async ({ page }) => {
    const walk = walkOf(page, await publishStressJourney(page));
    await widths(walk, [390]);
    expect(walk.found).toEqual([]);
    // A name that fits whole wears no mark.
    await journeyAt(page, "j_launch", "list");
    await expect(page.getByTestId("journey-name-cue")).toHaveCount(0);
  });
});
