// A journey's two pages and their addresses (2.2 to 2.4) on the in-browser host: every address
// the earlier screens had lands on its new page, the projection switcher keeps its place, a
// journey just started opens on the walkthrough, the address agents post for a node resolves to
// the page that shows it, and the toolbar's chips, their filters, and the keys act on the
// address.
import { expect, test } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { nextItem, nextKeys, openActing, turnOn } from "./acting.ts";
import { follow, goTo, nodePanel, openAt, openFilter, visit } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

const UP_FRONT = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];

// The address table is journeys/address.test.ts's; here the router really redirects an old
// canvas address, the walkthrough and the decisions page, and the root lands on Mine.
test("C11, C12: old addresses land on their new page, and the landing opens Mine", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_hiring?hide=action&heat=on");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/plan\/graph\?kind=group%2Cdecision%2Cdeliverable%2Cmilestone&lens=gravity$/);
  await visit(page, "browser", "/journeys/j_hiring/walkthrough");
  await expect(page.getByTestId("chip-decisions")).toHaveAttribute("aria-pressed", "true");
  await visit(page, "browser", "/journeys/j_hiring/decisions");
  await expect(page.getByTestId("decision-view")).toBeVisible();
  await visit(page, "browser", "/");
  await expect(page).toHaveURL(/\/mine$/);
  await expect(page.getByTestId("mine-journey").first()).toBeVisible();
});

test("C11: a journey just started opens on the walkthrough", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Landing"), { route: "vendor-evaluation", version: 1 });
  // The browser host's store lives as long as the page, so the journey is reached from within it.
  await follow(page, `/journeys/${id}`);
  await expect(page).toHaveURL(/\/next\/cards\?decisions=1$/);
});

test("the address agents post for a node opens the page that shows it, with its detail", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_hiring/nodes/n_offer");
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/next\/list\/nodes\/n_offer$/);
  await expect(nodePanel(page, "n_offer")).toBeVisible();
  await visit(page, "browser", "/journeys/j_vendor_eval/nodes/n_final_report");
  await expect(page).toHaveURL(/\/journeys\/j_vendor_eval\/plan\/graph\/nodes\/n_final_report\?open=n_final_review&trace=on$/);
  await expect(nodePanel(page, "n_final_report")).toBeVisible();
  await expect(page.locator('[data-testid="node-card"][data-node="n_final_report"]')).toBeVisible();
});

test("C10, C11: DECISIONS, the filter and its chips, the switcher and v act on the address; each page remembers its projection", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Chips"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", id, "next/list");
  expect(await nextKeys(page)).toContain("n_kickoff");
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => nextKeys(page).then((keys) => [...keys].sort())).toEqual(UP_FRONT);
  await expect(page.getByTestId("active-filters")).toHaveCount(0);

  // j opens the first row in the inspector, whose form is then the node's only one; Enter opens it.
  const [first = ""] = await nextKeys(page);
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("j");
  await expect(nodePanel(page, first)).toBeVisible();
  await expect(nextItem(page, first).getByTestId("acts")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(nodePanel(page, first).getByTestId("answer-editor")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByTestId("chip-decisions").click();
  await turnOn(page, "only-mine");
  await expect(page.getByTestId("filter-count")).toHaveText("1");
  await expect(page.getByTestId("active-filter")).toHaveText(/mine/);
  await page.getByTestId("active-filter").click();
  await expect(page.getByTestId("active-filters")).toHaveCount(0);
  await expect(page).not.toHaveURL(/mine=1/);

  await page.getByTestId("chip-decisions").click();
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(page.getByTestId("projection-cards")).toHaveAttribute("aria-current", "page");
  await expect(page).toHaveURL(/\/next\/cards\?decisions=1$/);
  await page.keyboard.press("v");
  await expect(page).toHaveURL(/\/next\/list\?decisions=1$/);

  await goTo(page, "plan", "list");
  await page.getByTestId("tab-next").click();
  await page.getByTestId("tab-plan").click();
  await expect(page.getByTestId("projection-list")).toHaveAttribute("aria-current", "page");

  // The tab and the projection you are on keep the address's settings when clicked.
  await openAt(page, "browser", id, "plan/graph?lens=gravity&kind=group%2Cdecision");
  await page.getByTestId("projection-graph").click();
  await page.getByTestId("tab-plan").click();
  await expect(page).toHaveURL(/\/plan\/graph\?lens=gravity&kind=group%2Cdecision$/);
});

test("the window does not scroll: the head stays, and a popover taller than the room scrolls inside itself", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  for (const address of ["next/list", "plan/list"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("journey-toolbar")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollHeight), address).toBe(800);
  }
  // A popover taller than the room below it scrolls inside itself, not the window.
  await page.setViewportSize({ width: 1280, height: 500 });
  await openAt(page, "browser", "j_vendor_eval", "plan/list", { fixedToday: FIXED_TODAY });
  await openFilter(page);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(500);
});

test("the projection switcher sits on the tabs' row, at the same right edge on every projection", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  const edges: Record<string, number> = {};
  for (const address of ["next/list", "next/cards", "plan/graph", "plan/list", "plan/timeline"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    const tabs = await page.getByTestId("journey-tabs").boundingBox();
    const switcher = await page.getByTestId("projection-switcher").boundingBox();
    expect(switcher?.y, address).toBeLessThan((tabs?.y ?? 0) + (tabs?.height ?? 0));
    edges[address] = Math.round((switcher?.x ?? 0) + (switcher?.width ?? 0));
  }
  expect(new Set(Object.values(edges)).size, JSON.stringify(edges)).toBe(1);
});

test("between 720 and 1100px the projection keeps the width and a canvas fills its region", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  const body = page.locator(".journey-body");
  for (const address of ["plan/graph", "plan/graph?decisions=1"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("canvas")).toBeVisible();
    const [scrolls, shown] = await body.evaluate((region) => [region.scrollHeight, region.clientHeight]);
    expect(scrolls, address).toBe(shown);
  }
  await openAt(page, "browser", "j_vendor_eval", "plan/list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  expect(await body.evaluate((region) => region.scrollWidth <= region.clientWidth)).toBe(true);
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("summary-upcoming")).toBeVisible();
});

test("the plan list's filter holds mine, kinds, flags and the owner; its sort is in the table's column headers", async ({ page }) => {
  await openActing(page, "browser", "j_vendor_eval", "plan/list?flag=next_up");
  await expect(page.getByTestId("list").getByRole("button", { name: "Sort by due" })).toBeVisible();
  await openFilter(page);
  const panel = page.getByTestId("filter-panel");
  for (const offered of ["flag-mine", "kind-milestone", "flag-overdue", "flag-snoozed"]) {
    await expect(panel.getByTestId(offered)).toBeVisible();
  }
  for (const gone of ["state-open", "flag-next_up"]) {
    await expect(panel.getByTestId(gone)).toHaveCount(0);
  }
  await expect(panel.getByLabel("Sort by")).toHaveCount(0);
  // The filter the popover no longer offers is still on, as a chip, and a flag chosen beside it keeps it.
  await turnOn(page, "flag-stale");
  await expect(page.getByTestId("active-filter")).toHaveText([/next up/, /stale/]);
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", "0");
});

test("NEXT's filter has the flags, and Rank for me is in its sort", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "next/list");
  expect(await nextKeys(page)).toHaveLength(5);
  await turnOn(page, "flag-shortfall");
  await expect.poll(() => nextKeys(page)).toEqual(["n_launch"]);
  await expect(page.getByTestId("filter-count")).toHaveText("1");
  await page.keyboard.press("Escape");
  await page.getByLabel("Sort by").selectOption({ label: "Rank for me" });
  await expect(page).toHaveURL(/\?flag=shortfall&me=1$/);
  await expect(page.getByTestId("filter-count")).toHaveText("1");
  await page.getByLabel("Sort by").selectOption({ label: "Gravity" });
  await expect(page).toHaveURL(/\?sort=gravity&flag=shortfall$/);
});

test("the journey card counts only what is still the viewer's to do", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "next/list", { fixedToday: FIXED_TODAY });
  const yours = page.getByTestId("card-yours");
  await expect(yours).toHaveText("You have 3 open items here, 2 ready");
  await expect(yours).toHaveAttribute("data-open", "3");
});
