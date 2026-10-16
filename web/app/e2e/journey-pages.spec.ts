// A journey's page in a real browser (2.2 to 2.4), on the in-browser host: what only focus,
// layout and the window show. The view state set on one view follows every other, and each
// draws the same node from it; the keys act on the view the pointer left them on (j, Enter,
// Esc, v); Frontier highlights without hiding; the search narrows every view; and no view
// scrolls the window: the head stays, a popover taller than the room scrolls inside itself, the
// toolbar keeps its row, and a canvas fills its region. The addresses and requests behind them
// are vitest's (journeys/view.test.ts and journeys/requests.test.ts).
import { expect, test, type Locator, type Page } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { viewportOf, viewportSettled } from "./canvas.ts";
import { daysAfter, frontierKeys, listKeys, passOrder, select, setFrontier } from "./acting.ts";
import { fresh, goTo, nodeCard, nodePanel, openAt, openFilter, openJourney, setDetail } from "./shell.ts";
import { closePopover, filterOf, ownAndPin, search, setCheck, setViewState, VIEWER } from "./view-state.ts";
import { FIXED_TODAY } from "./views.ts";

const UP_FRONT = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];

/** Computed opacity is not inherited, so the effective one is the product up the ancestors. */
function effective(target: Locator): Promise<number> {
  return target.evaluate((element) => {
    let product = 1;
    for (let at: Element | null = element; at !== null; at = at.parentElement) {
      product *= Number(getComputedStyle(at).opacity);
    }
    return product;
  });
}

/** Sorts the list by rank as a person does: the Rank column from Columns, then its header. */
async function sortByRank(page: Page): Promise<void> {
  await page.getByTestId("columns-button").click();
  await setCheck(page.getByTestId("column-rank"), true);
  await closePopover(page);
  await page.getByTestId("sort-rank").getByRole("button").click();
  await expect(page).toHaveURL(/sort=rank/);
}

/** A row of the list, by its node. */
function listRow(page: Page, node: string): Locator {
  return page.locator(`[data-testid="list-row"][data-node="${node}"]`);
}

/**
 * Makes X on a fresh journey's graph: an up-front decision on the frontier, given the viewer's
 * entity as owner and a pin two days back (so overdue), and the only node whose title holds the
 * word it returns.
 */
async function makeX(page: Page, id: string, x: string): Promise<string> {
  const word = fresh("w").split(" ")[1] ?? "";
  await openJourney(page, "browser", id);
  await ownAndPin(page, x, `Purchase or research ${word}`, daysAfter(FIXED_TODAY, -2));
  return word;
}

/** Sets every shared setting from the toolbar: Detail Decisions, Frontier, the whole Filter, and the search for `word`. */
const setEverything = (page: Page, word: string) => setViewState(page, "Decisions", { mine: true, owner: VIEWER, flags: ["overdue"], notRelevant: true, conditional: false }, word);

/** What `setEverything` writes to the address. */
const sharedOf = (word: string) => ["detail=decisions", "frontier=1", "mine=1", `owner=${VIEWER}`, "flag=overdue", "show=notrelevant", `q=${word}`];

/** The address carries every shared setting and the controls show each. */
async function holds(page: Page, view: string, word: string): Promise<void> {
  expect(new URL(page.url()).search.slice(1).split("&"), view).toEqual(expect.arrayContaining(sharedOf(word)));
  await expect(page.getByTestId("detail-button"), view).toHaveAccessibleName("Detail: Decisions");
  await expect(page.getByTestId("chip-frontier"), view).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("toolbar-search"), view).toHaveValue(word);
  await openFilter(page);
  const filter = filterOf(page);
  await expect(filter.getByTestId("flag-mine").locator("input"), view).toBeChecked();
  await expect(filter.getByTestId("filter-owner"), view).toHaveValue(VIEWER);
  await expect(filter.getByTestId("flag-overdue").locator("input"), view).toBeChecked();
  await expect(filter.getByTestId("show-not-relevant").locator("input"), view).toBeChecked();
  await expect(filter.getByTestId("show-conditional").locator("input"), view).not.toBeChecked();
  await closePopover(page);
}

/** The walk through, reached with w: X is its one card, and what it cannot honour keeps its setting, disabled. */
async function walkHolds(page: Page, x: string, word: string): Promise<void> {
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("w");
  await expect(page.getByTestId("walk-through")).toHaveAttribute("aria-current", "page");
  expect(await passOrder(page)).toEqual([x]);
  await holds(page, "walk", word);
  await expect(page.getByTestId("chip-frontier")).toBeDisabled();
  const before = page.url();
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  // Shift F does nothing here; d, which does, shows the keys are heard.
  await page.keyboard.press("Shift+F");
  await page.keyboard.press("d");
  await expect(page.getByTestId("chip-decisions")).toHaveAttribute("aria-pressed", "true");
  expect(new URL(page.url()).searchParams.get("frontier")).toBe("1");
  await page.keyboard.press("d");
  await expect(page).toHaveURL(before);
  await openFilter(page);
  await expect(filterOf(page).getByTestId("show-conditional").locator("input")).toBeDisabled();
  await expect(filterOf(page).getByTestId("show-not-relevant").locator("input")).toBeDisabled();
  await closePopover(page);
}

test("C2, C9, C10, C11: the view state set on the graph follows every view, and each draws the node it keeps", async ({ page }) => {
  await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
  const id = await startJourney(page, "browser", journeyName("Shared"), { route: "vendor-evaluation", version: 1 });
  const x = "n_purpose";
  const word = await makeX(page, id, x);
  await setEverything(page, word);
  await expect(nodeCard(page, x)).toBeVisible();
  await expect(page.getByTestId("node-card")).toHaveCount(1);
  await holds(page, "graph", word);

  await goTo(page, "list");
  await expect(listRow(page, x)).toBeVisible();
  await expect(page.getByTestId("list-row")).toHaveCount(1);
  await holds(page, "list", word);

  await goTo(page, "timeline");
  await expect(page.locator(`[data-testid="timeline-track"][data-node="${x}"]`)).toBeVisible();
  await holds(page, "timeline", word);
  // The timeline has no dates for work that does not apply: its Show not relevant keeps the setting, disabled.
  await openFilter(page);
  await expect(filterOf(page).getByTestId("show-not-relevant").locator("input")).toBeDisabled();
  await expect(filterOf(page).getByTestId("filter-disabled-reason")).toBeVisible();
  await closePopover(page);

  await goTo(page, "summary");
  await expect(page.getByTestId("summary-whole")).toBeVisible();
  expect(new URL(page.url()).search.slice(1).split("&")).toEqual(expect.arrayContaining(sharedOf(word)));

  await walkHolds(page, x, word);

  await goTo(page, "graph");
  await expect(nodeCard(page, x)).toBeVisible();
  await holds(page, "graph again", word);
});

test("C10, C11: after a click in the page, j opens the next row, Enter its form, Esc leaves it, and v swaps the view", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Keys"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", id, "list?frontier=1");
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => frontierKeys(page).then((keys) => [...keys].sort())).toEqual(UP_FRONT);

  // j opens the first row in the inspector, whose form is then the node's only one; Enter opens it.
  const [first = ""] = await listKeys(page);
  expect(UP_FRONT).toContain(first);
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("j");
  await expect(nodePanel(page, first)).toBeVisible();
  await expect(listRow(page, first).getByTestId("acts")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(nodePanel(page, first).getByTestId("answer-editor")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(page.getByTestId("view-timeline")).toHaveAttribute("aria-current", "page");
  await expect(page).toHaveURL(/\/timeline\?frontier=1&decisions=1$/);
  await page.keyboard.press("v");
  await expect(page).toHaveURL(/\/graph\?frontier=1&decisions=1$/);
});

test("C10: Frontier highlights the list: every row stays, the frontier's rows carry their action under the title, and the rest fade", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const everything = await listKeys(page);
  await setFrontier(page, true);
  await expect(page).toHaveURL(/\/list\?frontier=1$/);
  expect(await listKeys(page)).toEqual(everything);
  const counted = /(\d+) can move now/.exec((await page.getByTestId("chip-frontier").getAttribute("title")) ?? "")?.[1];
  const frontier = await frontierKeys(page);
  expect(String(frontier.length)).toBe(counted);
  // A frontier row says what to do under its title, at a phone's width too.
  const [moving = ""] = frontier;
  const title = listRow(page, moving).locator('td[data-column="title"]');
  const acting = title.getByTestId("list-acting");
  await expect(acting).toBeVisible();
  const [named, act] = await Promise.all([title.locator("a").first().boundingBox(), acting.boundingBox()]);
  expect(act?.y ?? 0).toBeGreaterThan(named?.y ?? 0);
  // A row the frontier does not light fades its content, and stays faded when selected.
  const still = page.locator('[data-testid="list-row"][data-lit="false"]').first();
  const content = still.locator('td[data-column="title"] .list-content');
  expect(await effective(content)).toBeCloseTo(0.3);
  await select(page, (await still.getAttribute("data-node")) ?? "");
  expect(await effective(content)).toBeCloseTo(0.3);
  // Sorting by Rank is the frontier's order: the rank column counts up.
  await page.setViewportSize({ width: 1280, height: 800 });
  await sortByRank(page);
  await expect(page.getByTestId("list-table")).toHaveAttribute("data-ranked", "true");
  await expect
    .poll(async () => {
      const places = (await page.locator('[data-testid="list-row"] td[data-column="rank"]').allTextContents()).filter((text) => text.startsWith("#"));
      return places.length > 0 && places.every((place, at) => place === `#${String(at + 1)}`);
    })
    .toBe(true);
});

test("C2, C9: a search keeps the containers of what matches: at Deliverables an action's match keeps its deliverable's row, in the tree and sorted", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list?detail=work", { fixedToday: FIXED_TODAY });
  await expect(listRow(page, "n_access")).toBeVisible();
  await search(page, "Draft the plan");
  await expect(listRow(page, "n_access")).toHaveCount(0);
  await expect(listRow(page, "n_plan")).toBeVisible();
  await expect(listRow(page, "n_plan_draft")).toHaveCount(0);
  await sortByRank(page);
  await expect(listRow(page, "n_plan")).toBeVisible();
  await expect(listRow(page, "n_access")).toHaveCount(0);
});

test("C2, C9: a search narrows the graph to its matches and fits them; nothing matching offers Reset the view, which clears the search and the collapses", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  await viewportSettled(page);
  const whole = await page.getByTestId("node-card").count();
  const fitted = await viewportOf(page);
  await search(page, "Partner");
  await expect(nodeCard(page, "n_partner_runs")).toBeVisible();
  await expect(nodeCard(page, "n_access")).toHaveCount(0);
  await viewportSettled(page);
  expect(await viewportOf(page)).not.toEqual(fitted);
  await search(page, "");
  await expect(page).toHaveURL(/\/graph$/);
  await expect(page.getByTestId("node-card")).toHaveCount(whole);
  // A search longer than the engine reads never leaves the box.
  await search(page, "a".repeat(257));
  expect(await page.getByTestId("toolbar-search").evaluate((box: HTMLInputElement) => box.validity.valid)).toBe(false);
  await expect(page).toHaveURL(/\/graph$/);

  await openAt(page, "browser", "j_vendor_eval", "graph?q=nothing-is-called-this&shut=n_setup");
  await expect(page.getByTestId("nothing-matches")).toBeVisible();
  await page.getByTestId("nothing-matches").getByTestId("reset-view").click();
  await expect(page).toHaveURL(/\/graph$/);
  await expect(nodeCard(page, "n_access")).toBeVisible();
});

test("C2: a fold holds until a step is picked: back at All after another step, every level is open again", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const fold = () => listRow(page, "n_setup").getByTestId("list-fold");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await expect(listRow(page, "n_access")).toHaveCount(0);
  await setDetail(page, "Deliverables");
  await setDetail(page, "All");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await expect(listRow(page, "n_access")).toBeVisible();
});

test("C2: picking the step already chosen, or Reset the view, drops the list's and the timeline's folds", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const fold = () => listRow(page, "n_setup").getByTestId("list-fold");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await setDetail(page, "All");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await search(page, "nothing-is-called-this");
  await page.getByTestId("list-empty").getByTestId("reset-view").click();
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
});

test("C9, C12: the list's Decisions with a search no decision matches is empty, and Reset the view brings the rows back", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list?decisions=1&q=Kickoff", { fixedToday: FIXED_TODAY });
  const empty = page.getByTestId("list-empty");
  await expect(empty).toBeVisible();
  await expect(page.getByTestId("list-row")).toHaveCount(0);
  await empty.getByTestId("reset-view").click();
  await expect(page).toHaveURL(/\/list$/);
  await expect(page.getByTestId("list-row").first()).toBeVisible();
});

test("a journey opens as the stock view, switching views keeps the chips, and coming back to the journey starts over", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  const detail = page.getByTestId("detail-button");
  // The stock view: every detail, fitted to the whole graph.
  await expect(detail).toHaveAccessibleName("Detail: All");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await viewportSettled(page);
  const opened = await viewportOf(page);
  await page.keyboard.press("f");
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(opened);
  // A chip follows a switch of view and back; the pan is the view's own.
  await page.getByTestId("chip-frontier").click();
  await page.mouse.move(600, 500);
  await page.mouse.wheel(0, 150);
  await viewportSettled(page);
  await page.getByTestId("view-list").click();
  await expect(page).toHaveURL(/\/list\?frontier=1$/);
  await page.getByTestId("view-graph").click();
  await expect(page).toHaveURL(/\/graph\?frontier=1$/);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(opened);
  // Back at the journey's landing, nothing is held: no chip, no pan.
  await page.evaluate(() => {
    history.pushState(null, "", location.pathname.replace(/\/graph.*/, ""));
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page).toHaveURL(/\/graph$/);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(detail).toHaveAccessibleName("Detail: All");
});

test("the window does not scroll: the head stays, a popover scrolls inside itself, the toolbar keeps its row with Walk through at one edge, and a canvas fills its region", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  for (const address of ["list?frontier=1", "list"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("journey-toolbar")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollHeight), address).toBe(800);
  }
  // A popover taller than the room below it scrolls inside itself, not the window.
  await page.setViewportSize({ width: 1280, height: 500 });
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await openFilter(page);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(500);
  // Walk through sits on the views' row, at the same right edge on every view, between 720 and 1100px.
  await page.setViewportSize({ width: 1024, height: 700 });
  const edges: Record<string, number> = {};
  for (const address of ["walk", "graph", "list", "timeline"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    const views = await page.getByTestId("journey-views").boundingBox();
    const walk = await page.getByTestId("walk-through").boundingBox();
    expect(walk?.y, address).toBeLessThan((views?.y ?? 0) + (views?.height ?? 0));
    edges[address] = Math.round((walk?.x ?? 0) + (walk?.width ?? 0));
  }
  expect(new Set(Object.values(edges)).size, JSON.stringify(edges)).toBe(1);
  // Between 720 and 1100px the projection keeps the width and a canvas fills its region.
  const body = page.locator(".journey-body");
  for (const address of ["graph", "graph?decisions=1"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("canvas")).toBeVisible();
    const [scrolls, shown] = await body.evaluate((region) => [region.scrollHeight, region.clientHeight]);
    expect(scrolls, address).toBe(shown);
  }
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  expect(await body.evaluate((region) => region.scrollWidth <= region.clientWidth)).toBe(true);
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("summary-upcoming")).toBeVisible();
});

test("a done row fades its content like a done card, lifts the fade under keyboard focus, and leaves the checkbox at full strength", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list");
  const done = page.locator('[data-testid="list-row"][data-state="done"]').first();
  const title = done.locator('td[data-column="title"] a').first();
  expect(await effective(title)).toBe(0.5);
  // The checkbox shows on hover.
  await done.hover();
  expect(await effective(done.locator("td.list-check input"))).toBe(1);
  await title.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  expect(await effective(title)).toBe(1);
});
