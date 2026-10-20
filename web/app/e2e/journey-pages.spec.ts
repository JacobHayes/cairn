// A journey's page in a real browser (2.2 to 2.4), on the in-browser host: what only focus,
// layout and the window show. The view state set on one view follows every other, and each
// draws the same node from it; the keys act on the view the pointer left them on (j, Enter,
// Esc, v); Frontier highlights without hiding; the search narrows every view; and no view
// scrolls the window: the head stays, a popover taller than the room scrolls inside itself, the
// View picker keeps its place in the header, and a canvas fills its region. The View picker lands
// on each view, by pointer and by keys, and the journey's status moves from its `⋯`. The addresses and requests behind them
// are vitest's (journeys/view.test.ts and journeys/requests.test.ts). Depth and its Fade follow
// every view, Fade moves nothing, and the walk through at Decisions is the decision walkthrough.
import { type Locator, type Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { journeyName, setStatus, startJourney } from "./around.ts";
import { viewportOf, viewportSettled } from "./canvas.ts";
import { daysAfter, frontierKeys, listKeys, passOrder, select, setFrontier, withFrontier } from "./acting.ts";
import { expectAddress, fresh, goTo, nodeCard, nodePanel, openAt, openFilter, openJourney, setDepth, setFade } from "./shell.ts";
import { closePopover, filterOf, ownAndPin, search, setCheck, setViewState, VIEWER } from "./view-state.ts";
import { FIXED_TODAY } from "./views.ts";

/** The journey's own path, without the query: the tab's page is a journey page. */
function journeyBase(page: Page): string {
  return new URL(page.url()).pathname;
}

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
 * identity as owner and a pin two days back (so overdue), and the only node whose title holds the
 * word it returns.
 */
async function makeX(page: Page, id: string, x: string): Promise<string> {
  const word = fresh("w").split(" ")[1] ?? "";
  await openJourney(page, "browser", id);
  await ownAndPin(page, x, `Purchase or research ${word}`, daysAfter(FIXED_TODAY, -2));
  return word;
}

/** Sets every shared setting from the toolbar: Depth Decisions, Frontier, the whole Filter, and the search for `word`. */
const setEverything = (page: Page, word: string) => setViewState(page, "Decisions", { mine: true, owner: VIEWER, flags: ["overdue"], notRelevant: true, conditional: false }, word);

/** What `setEverything` writes to the address. */
const sharedOf = (word: string) => ["depth=decisions", "frontier=1", "mine=1", `owner=${VIEWER}`, "flag=overdue", "show=notrelevant", `q=${word}`];

/** The address carries every shared setting and the controls show each. */
async function holds(page: Page, view: string, word: string): Promise<void> {
  expect(new URL(page.url()).search.slice(1).split("&"), view).toEqual(expect.arrayContaining(sharedOf(word)));
  await expect(page.getByTestId("depth-button"), view).toHaveAccessibleName("Depth: Decisions");
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

/** The Walkthrough, reached with w: X is its one card, and what it cannot honour keeps its setting, disabled. */
async function walkHolds(page: Page, x: string, word: string): Promise<void> {
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("w");
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "walk");
  expect(await passOrder(page)).toEqual([x]);
  await holds(page, "walk", word);
  await expect(page.getByTestId("chip-frontier")).toBeDisabled();
  const canon = (address: string) => {
    const url = new URL(address);
    url.searchParams.sort();
    return url.href;
  };
  const before = canon(page.url());
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  // Shift F does nothing here; m, which does, shows the keys are heard.
  await page.keyboard.press("Shift+F");
  await page.keyboard.press("m");
  await expect.poll(() => new URL(page.url()).searchParams.get("mine")).toBeNull();
  expect(new URL(page.url()).searchParams.get("frontier")).toBe("1");
  await page.keyboard.press("m");
  await expect.poll(() => canon(page.url())).toBe(before);
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
  await setDepth(page, "Decisions");
  // Decisions depth draws the milestones too: the frontier is the up-front decisions and the milestones that need nothing.
  await expect.poll(() => frontierKeys(page).then((keys) => keys.filter((key) => key !== "n_kickoff" && key !== "n_decision_meeting").sort())).toEqual(UP_FRONT);

  // j opens the row after the selected one in the inspector (the first, with none), whose form is then the node's only one; Enter opens it.
  const rows = await listKeys(page);
  const decided = rows.findIndex((key) => UP_FRONT.includes(key));
  expect(decided).toBeGreaterThanOrEqual(0);
  const first = rows[decided] ?? "";
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  for (let at = 0; at <= decided; at += 1) {
    await page.keyboard.press("j");
    await expect(nodePanel(page, rows[at] ?? "")).toBeVisible();
  }
  await expect(nodePanel(page, first)).toBeVisible();
  await expect(listRow(page, first).getByTestId("acts")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(nodePanel(page, first).getByTestId("answer-editor")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expectAddress(page, `${journeyBase(page)}?view=timeline&depth=decisions&frontier=1`);
  // The next v acts on the view drawn, so wait for it.
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "timeline");
  await page.keyboard.press("v");
  await expectAddress(page, `${journeyBase(page)}?view=graph&depth=decisions&frontier=1`);
});

test("C10: Frontier highlights the list: every row stays, the frontier's rows carry their action under the title, and the rest fade", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const everything = await listKeys(page);
  await setFrontier(page, true);
  await expectAddress(page, `${journeyBase(page)}?view=list&frontier=1`);
  expect(await listKeys(page)).toEqual(everything);
  const counted = /(\d+) can move now/.exec((await withFrontier(page, (chip) => chip.getAttribute("title"))) ?? "")?.[1];
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
  const still = page.locator('[data-testid="list-row"][data-faded="true"]').first();
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
  await openAt(page, "browser", "j_vendor_eval", "list?depth=deliverables", { fixedToday: FIXED_TODAY });
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
  await expectAddress(page, `${journeyBase(page)}?view=graph`);
  await expect(page.getByTestId("node-card")).toHaveCount(whole);
  // A search longer than the engine reads never leaves the box.
  await search(page, "a".repeat(257));
  expect(await page.getByTestId("toolbar-search").evaluate((box: HTMLInputElement) => box.validity.valid)).toBe(false);
  await expectAddress(page, `${journeyBase(page)}?view=graph`);

  await openAt(page, "browser", "j_vendor_eval", "graph?q=nothing-is-called-this&shut=n_setup");
  await expect(page.getByTestId("nothing-matches")).toBeVisible();
  await page.getByTestId("nothing-matches").getByTestId("reset-view").click();
  await expectAddress(page, `${journeyBase(page)}?view=graph`);
  await expect(nodeCard(page, "n_access")).toBeVisible();
});

test("C2: a fold holds until a depth is picked: back at Actions after another depth, every level is open again", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const fold = () => listRow(page, "n_setup").getByTestId("list-fold");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await expect(listRow(page, "n_access")).toHaveCount(0);
  await setDepth(page, "Deliverables");
  await setDepth(page, "Actions");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await expect(listRow(page, "n_access")).toBeVisible();
});

test("C2: picking the depth already chosen, or Reset the view, drops the list's and the timeline's folds", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  const fold = () => listRow(page, "n_setup").getByTestId("list-fold");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await setDepth(page, "Actions");
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
  await fold().click();
  await expect(fold()).toHaveAttribute("aria-expanded", "false");
  await search(page, "nothing-is-called-this");
  await page.getByTestId("list-empty").getByTestId("reset-view").click();
  await expect(fold()).toHaveAttribute("aria-expanded", "true");
});

test("C2, C9: the list at Decisions with a search that matches nothing is empty, and Reset the view brings back Actions depth and the rows", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list?depth=decisions&q=nothing-is-called-this", { fixedToday: FIXED_TODAY });
  const empty = page.getByTestId("list-empty");
  await expect(empty).toBeVisible();
  await expect(page.getByTestId("list-row")).toHaveCount(0);
  await empty.getByTestId("reset-view").click();
  await expectAddress(page, `${journeyBase(page)}?view=list`);
  await expect(page.getByTestId("list-row").first()).toBeVisible();
});

test("a journey opens as the stock view, switching views keeps the chips, and coming back to the journey starts over", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  const depth = page.getByTestId("depth-button");
  // The stock view: every depth, fitted to the whole graph.
  await expect(depth).toHaveAccessibleName("Depth: Actions");
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
  await goTo(page, "list");
  await expectAddress(page, `${journeyBase(page)}?view=list&frontier=1`);
  await goTo(page, "graph");
  await expectAddress(page, `${journeyBase(page)}?view=graph&frontier=1`);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(opened);
  // Back at the journey's landing, nothing is held: no chip, no pan.
  await page.evaluate(() => {
    history.pushState(null, "", location.pathname);
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "summary");
  expect(new URL(page.url()).search).toBe("");
  await goTo(page, "graph");
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(depth).toHaveAccessibleName("Depth: Actions");
});

test("the window does not scroll: the head stays, a popover scrolls inside itself, the View picker keeps its place on the title's row, and a canvas fills its region", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  for (const address of ["list?frontier=1", "list"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("journey-header")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollHeight), address).toBe(800);
  }
  // A popover taller than the room below it scrolls inside itself, not the window.
  await page.setViewportSize({ width: 1280, height: 500 });
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await openFilter(page);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(500);
  // The View picker sits on the title's row, at the same right edge on every view, between 720 and 1100px.
  await page.setViewportSize({ width: 1024, height: 700 });
  const edges: Record<string, number> = {};
  for (const address of ["walk", "graph", "list", "timeline", "summary"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    const title = await page.getByTestId("journey-name").boundingBox();
    const picker = await page.getByTestId("view-picker").boundingBox();
    expect(picker?.y, address).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
    edges[address] = Math.round((picker?.x ?? 0) + (picker?.width ?? 0));
  }
  expect(new Set(Object.values(edges)).size, JSON.stringify(edges)).toBe(1);
  // Between 720 and 1100px the projection keeps the width and a canvas fills its region.
  const body = page.locator(".journey-body");
  for (const address of ["graph", "graph?depth=decisions"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("canvas")).toBeVisible();
    const [scrolls, shown] = await body.evaluate((region) => [region.scrollHeight, region.clientHeight]);
    expect(scrolls, address).toBe(shown);
  }
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  expect(await body.evaluate((region) => region.scrollWidth <= region.clientWidth)).toBe(true);
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("summary-milestones")).toBeVisible();
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

/** Whether the Depth menu's Fade checkbox is ticked, read from the open menu; none when the menu has no Fade (the walk). */
async function fadeTicked(page: Page): Promise<boolean | undefined> {
  await page.getByTestId("depth-button").click();
  const check = page.getByTestId("depth-fade");
  const ticked = (await check.count()) === 0 ? undefined : await check.getByRole("checkbox").isChecked();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("depth-panel")).toHaveCount(0);
  return ticked;
}

test("C2, C11, C12: the depth and Fade follow every view; the walk through has no Fade and at Decisions walks decisions; the list gains the decision columns", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Depth"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", id, "graph");
  await setDepth(page, "Decisions");
  await setFade(page, true);
  // Fade draws the deliverable and dims it; it is a card on the canvas.
  await expect(nodeCard(page, "n_plan")).toHaveClass(/node-faded/);
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  const depthed = (view: string) => { expect(new URL(page.url()).searchParams.get("depth"), view).toBe("decisions"); expect(new URL(page.url()).searchParams.get("fade"), view).toBe("1"); };
  depthed("graph");

  await goTo(page, "list");
  depthed("list");
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Decisions");
  expect(await fadeTicked(page)).toBe(true);
  await expect(page.locator('th[data-column="answer"]')).toBeVisible();
  await expect(page.locator('[data-testid="list-row"][data-node="n_plan"]')).toHaveAttribute("data-faded", "true");
  await expect(page.locator('[data-testid="list-row"][data-node="n_partner_runs"]')).not.toHaveAttribute("data-faded", "true");

  await goTo(page, "timeline");
  depthed("timeline");
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Decisions");
  expect(await fadeTicked(page)).toBe(true);

  await goTo(page, "summary");
  depthed("summary");
  await expect(page.getByTestId("depth-button")).toHaveCount(0);

  await goTo(page, "walk");
  depthed("walk");
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Decisions");
  expect(await fadeTicked(page)).toBeUndefined();
  // The decision walkthrough: its card is a decision, and the milestone on the frontier is not walked.
  await expect(page.getByTestId("triage")).toHaveAttribute("aria-label", "Decision walkthrough");
  await expect(page.getByTestId("triage-card")).toHaveAttribute("data-kind", "decision");
  expect(await passOrder(page)).not.toContain("n_kickoff");

  await goTo(page, "graph");
  depthed("graph again");
  await expect(nodeCard(page, "n_plan")).toHaveClass(/node-faded/);
  expect(await fadeTicked(page)).toBe(true);
});

test("C2: Fade moves nothing: stepping the depth with Fade on keeps every card and the camera where Actions has them, and Hide returns to a layout of its own", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  await viewportSettled(page);
  const cards = page.getByTestId("node-card");
  const place = () => nodeCard(page, "n_kickoff").evaluate((card) => [card.getAttribute("data-x"), card.getAttribute("data-y")]);
  const [count, there, camera] = [await cards.count(), await place(), await viewportOf(page)];
  await setFade(page, true);
  for (const depth of ["Decisions", "Stages", "Deliverables", "Actions"]) {
    await setDepth(page, depth);
    await expect(cards, depth).toHaveCount(count);
    expect(await place(), depth).toEqual(there);
    await viewportSettled(page);
    expect(await viewportOf(page), depth).toEqual(camera);
  }
  // Hide at a shallower depth draws fewer cards; Fade again draws the Actions layout where it was.
  await setDepth(page, "Decisions");
  await setFade(page, false);
  await expect.poll(() => cards.count()).toBeLessThan(count);
  await setFade(page, true);
  await expect(cards).toHaveCount(count);
  expect(await place()).toEqual(there);
  // A stage the viewer collapsed stays collapsed under Fade, where it was.
  await setDepth(page, "Actions");
  await nodeCard(page, "n_setup").getByTestId("card-collapse").dispatchEvent("click");
  await expect(page).toHaveURL(/shut=n_setup/);
  await expect.poll(() => cards.count()).toBeLessThan(count);
  const collapsed = await cards.count();
  await setFade(page, false);
  await setFade(page, true);
  await expect(cards).toHaveCount(collapsed);
  // Stepping the depth, by menu or by key, under Fade only dims: the fold stays too.
  await setDepth(page, "Decisions");
  await expect(page).toHaveURL(/shut=n_setup/);
  await page.keyboard.press("3");
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Deliverables");
  await expect(page).toHaveURL(/shut=n_setup/);
  await expect(cards).toHaveCount(collapsed);
});

test("C11: the walk through at Decisions walks the up-front decisions and not the kickoff milestone; at Actions it walks the milestone; with every decision made it says so, and Continue leaves Decisions", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Walk"), { route: "vendor-evaluation", version: 1 });
  await expect(page.getByTestId("triage")).toHaveAttribute("data-order", /\bn_purpose\b/);
  expect(await passOrder(page)).not.toContain("n_kickoff");
  expect(await passOrder(page)).toEqual(expect.arrayContaining(UP_FRONT.filter((key) => key !== "n_meeting_date")));
  await setDepth(page, "Actions");
  await expect.poll(() => passOrder(page)).toContain("n_kickoff");
  expect(id).toMatch(/^j_/);

  // The seeded vendor evaluation has every decision made: nothing to decide, and Continue goes on to everything.
  await openAt(page, "browser", "j_vendor_eval", "walk?depth=decisions", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("no-decisions")).toBeVisible();
  await page.getByRole("button", { name: "Continue with everything" }).click();
  await expectAddress(page, `${journeyBase(page)}?view=walk`);
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Actions");
});

/** The view root each screen draws, which is there even when the view has no rows. */
const ROOTS: Record<string, string> = { graph: "canvas", list: "list-total", timeline: "timeline", summary: "summary", walk: "triage" };

test("C9-C13: the View picker lands on each view, keeps the shared settings, and says which one is open", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "graph?frontier=1", { fixedToday: FIXED_TODAY });
  for (const view of ["list", "timeline", "summary", "walk", "graph"]) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${view}`).click();
    await expectAddress(page, `/journeys/j_vendor_eval?${view === "summary" ? "" : `view=${view}&`}frontier=1`);
    await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", view);
    await expect(page.getByTestId(ROOTS[view] ?? "")).toBeVisible();
    // Opened again, exactly one item is marked as where you are, and it is this view's.
    await page.getByTestId("view-picker").click();
    const current = page.getByTestId("view-picker-menu").locator('[aria-current="page"]');
    await expect(current).toHaveCount(1);
    await expect(current).toHaveAttribute("data-testid", `view-${view}`);
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("view-picker-menu")).toHaveCount(0);
  }
});

test("C9, C11: the View picker works from the keyboard: it opens on the current view, the arrows wrap, and Esc, Enter and Tab hand the focus back", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "graph", { fixedToday: FIXED_TODAY });
  const button = page.getByTestId("view-picker");
  await button.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("view-graph")).toBeFocused();
  // Up from the first item wraps to the last, the Walkthrough; down twice from there is the List.
  await page.keyboard.press("ArrowUp");
  await expect(page.getByTestId("view-walk")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await expect(page.getByTestId("view-list")).toBeFocused();
  await page.keyboard.press("Enter");
  await expectAddress(page, `${journeyBase(page)}?view=list`);
  // The router's transition may still be pending at the address: the picker names the view once it has landed.
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "list");
  await expect(button).toBeFocused();
  // Esc closes it and keeps the view.
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("view-list")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("view-picker-menu")).toHaveCount(0);
  await expect(button).toBeFocused();
  // Choosing the view already open changes nothing and keeps the focus on the button.
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expectAddress(page, `${journeyBase(page)}?view=list`);
  await expect(page.getByTestId("view-picker-menu")).toHaveCount(0);
  await expect(button).toBeFocused();
  // Tab closes it and moves on to the next control, the journey's menu.
  await page.keyboard.press("Enter");
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("view-picker-menu")).toHaveCount(0);
  await expect(page.getByTestId("journey-menu")).toBeFocused();
});

test("B11: the menu's Status opens from the keyboard, and Esc leaves the status as it was", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "graph", { fixedToday: FIXED_TODAY });
  await page.getByTestId("journey-menu").focus();
  await page.keyboard.press("Enter");
  await page.keyboard.press("End");
  const status = page.getByTestId("menu-status");
  await expect(status).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(status).toHaveAttribute("aria-expanded", "true");
  await expect(status).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByTestId("status-completed")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("journey-menu-menu")).toHaveCount(0);
  await expect(page.getByTestId("journey-menu")).toBeFocused();
  await expect(page.getByTestId("journey-status")).toHaveCount(0);
});

test("B11, A19, C16: the status moves from the menu's Status, and the header says when the journey is not active", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "graph", { fixedToday: FIXED_TODAY });
  const menu = page.getByTestId("journey-menu");
  const status = page.getByTestId("menu-status");
  const chip = page.getByTestId("journey-status");
  // Active says nothing beside the name; the Status row says so, closed, and opens to the moves.
  await expect(chip).toHaveCount(0);
  await menu.click();
  await expect(status).toHaveAttribute("data-status", "active");
  await expect(status).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByTestId("status-completed")).toHaveCount(0);
  await status.click();
  await expect(page.getByTestId("status-completed")).toBeVisible();
  await expect(page.getByTestId("status-archived")).toBeVisible();
  await expect(page.getByTestId("status-delete")).toHaveCount(0);
  await page.getByTestId("status-archived").click();
  await expect(chip).toHaveAttribute("data-status", "archived");
  // Archived: no Edit, a way back, and deletion.
  await menu.click();
  await expect(page.getByTestId("menu-edit")).toHaveCount(0);
  await status.click();
  await expect(page.getByTestId("status-completed")).toBeVisible();
  await expect(page.getByTestId("status-delete")).toBeVisible();
  await page.getByTestId("status-completed").click();
  await expect(chip).toHaveAttribute("data-status", "completed");
  await setStatus(page, "active");
  await expect(chip).toHaveCount(0);
  // Deletion is asked for once archived.
  await setStatus(page, "archived");
  await menu.click();
  await status.click();
  await page.getByTestId("status-delete").click();
  await expect(page.getByTestId("delete-journey")).toBeVisible();
});

test("C13: a pan does not come back when the range is chosen away and back", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "timeline");
  await page.getByTestId("timeline").hover();
  await page.mouse.wheel(300, 0);
  await expect(page.getByTestId("timeline-reset")).toBeVisible();
  await page.getByTestId("timeline-range").getByText("Month", { exact: true }).click();
  await page.getByTestId("timeline-range").getByText("Fit", { exact: true }).click();
  await expect(page.getByTestId("timeline-reset")).toHaveCount(0);
});
