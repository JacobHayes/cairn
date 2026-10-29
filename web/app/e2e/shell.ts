// What the app's browser tests share: opening a screen on a host, and reading and editing a
// journey page the way a person does: a node's card on the canvas, its detail beside it.
import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";

/**
 * Opens the screen at `path` (`/journeys/<id>?view=...`) once the shell is up: within the page
 * when it already runs the app, as a link would; by loading the page otherwise. Returns once the
 * page has settled.
 */
export async function open(page: Page, path: string): Promise<void> {
  const nav = page.getByRole("navigation", { name: "Screens", exact: true });
  // eslint-disable-next-line e2e/settled-reads -- whether the app runs in this tab at all: a blank tab has nothing to settle
  if ((await nav.count()) === 1) {
    await goWithin(page, path);
  } else {
    await page.goto(path);
  }
  await expect(nav).toBeVisible();
  await settled(page);
}

/** From the new-route form (`/routes/new`), imports a route file: a new route, or a new draft of the one it names. */
export async function importRoute(page: Page, file: string | { name: string; mimeType: string; buffer: Buffer }): Promise<void> {
  await open(page, "/routes/new");
  await page.getByLabel("Import a file…").setInputFiles(file);
}

/** The sync dot at the right edge of the brand row at the top of the rail. */
export function syncChip(page: Page): Locator {
  return page.getByTestId("sync");
}

/**
 * Whether the page is settled now and still is after two more turns of its event loop: the
 * render that settled it may run effects that start more work (a screen keys its reads in one).
 * Runs in the page; a turn is a message, which no fake clock holds back.
 */
async function stillSettled(): Promise<boolean> {
  const quiet = () => document.querySelector('[data-testid="sync"]')?.getAttribute("data-settled") === "true";
  const turn = () =>
    new Promise((resolve) => {
      const channel = new MessageChannel();
      channel.port1.onmessage = resolve;
      channel.port2.postMessage(null);
    });
  if (!quiet()) {
    return false;
  }
  await turn();
  await turn();
  return quiet();
}

/**
 * Waits until nothing the page has under way will change what it shows: no read or screen at
 * work, no write in flight, the stream open (the sync dot's `data-settled`). What changes by the
 * clock alone (SAVED fading, a toast) is not waited for: a test that needs it runs the page's
 * clock (`page.clock`).
 */
export async function settled(page: Page): Promise<void> {
  await expect.poll(() => page.evaluate(stillSettled), { intervals: [16] }).toBe(true);
}

/**
 * `read` of the page once it has settled. A value read from the page at any other moment may be
 * one it is about to replace; eslint allows a read that does not retry only here, in
 * `expect.poll` and in `toPass`.
 */
export async function whenSettled<T>(page: Page, read: () => Promise<T>): Promise<T> {
  await settled(page);
  return await read();
}

/** Waits until the screen's journey is derived: its name is up, the chip knows its revision, and the page has settled. */
export async function derived(page: Page): Promise<void> {
  await expect(page.getByTestId("journey-name")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-revision", /^\d+$/);
  await settled(page);
}

/** Waits until the stream is live and nothing is pending: the chip says "In sync, live". */
export async function live(page: Page): Promise<void> {
  await expect(syncChip(page)).toHaveAttribute("aria-label", /In sync · live/);
}

/** The popover's Recent, newest first, once it lists a save; the popover is closed again. */
export async function recentSaves(page: Page): Promise<string[]> {
  const chip = syncChip(page);
  if ((await whenSettled(page, () => chip.getAttribute("aria-expanded"))) !== "true") {
    await chip.click();
  }
  const recent = page.getByTestId("sync-popover").getByTestId("sync-recent");
  await expect(recent.first()).toBeVisible();
  const saves = await whenSettled(page, () => recent.allTextContents());
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sync-popover")).toHaveCount(0);
  return saves;
}

/** Loads the screen at `path` afresh, as a bookmark or a posted link does, and returns once the shell is up and the page has settled. */
export async function visit(page: Page, path: string): Promise<void> {
  await page.goto(path);
  await expect(page.getByRole("navigation", { name: "Screens", exact: true })).toBeVisible();
  await settled(page);
}

/** Moves the tab to `path` as a link does, without loading the page again, and without waiting for where it lands. */
export async function follow(page: Page, path: string): Promise<void> {
  await page.evaluate((to) => {
    history.pushState(null, "", to);
    dispatchEvent(new PopStateEvent("popstate"));
  }, path);
}

/**
 * Moves the tab to the screen at `path` without loading the page again, as the app's links do,
 * and returns once the router has rendered it (the shell's `data-screen`) and the page has
 * settled on it.
 */
export async function goWithin(page: Page, path: string): Promise<void> {
  await follow(page, path);
  const main = page.locator("main[data-screen]");
  await expect.poll(async () => screenOf((await main.getAttribute("data-screen")) ?? "")).toBe(screenOf(path));
  await settled(page);
}

/** Waits until the tab is at `address`, whatever the order of its query parameters. */
export async function expectAddress(page: Page, address: string): Promise<void> {
  await expect.poll(() => screenOf(page.url())).toBe(screenOf(address));
}

/** The screen an address names: its path and its query, in a stable order. */
function screenOf(address: string): string {
  const url = new URL(address, "http://screen.invalid");
  url.searchParams.sort();
  return `${url.pathname}?${url.searchParams.toString()}`;
}

/**
 * A journey's graph, once derived and its canvas drawn. The graph opens at its Actions
 * depth unless `query` says otherwise, so every node has a card to reach (it opens at Stages, with
 * most stages folded, for a person).
 */
export async function openJourney(page: Page, journey: string, query = "?depth=actions"): Promise<void> {
  await open(page, `/journeys/${journey}?view=graph${query === "" ? "" : `&${query.replace(/^\?/, "")}`}`);
  await derived(page);
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  // Once the opening fit has moved the viewport off its start, so it cannot land after what the spec does next.
  await page.waitForFunction(() => document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform !== "translate(0px, 0px) scale(1)");
  // The default opens the whole graph at every depth, fitted, so a spec reaches any node.
  if (query !== "") {
    await page.keyboard.press("f");
  }
}

/**
 * The address of journey `journey`'s page at `address`: the view, then `/edges/<e>` for an edge's
 * card, then a query (`list?frontier=1`, `graph/edges/a~b`, `summary`); with node `node` open when
 * given. It always names the view, so the app is given what the test means whatever the default is.
 */
export function journeyAddress(journey: string, address: string, node?: string): string {
  const [path = "", query] = address.split("?");
  const [view = "", ...place] = path.split("/");
  const at = node === undefined ? (place.length === 0 ? "" : `/${place.join("/")}`) : `/nodes/${node}`;
  return `/journeys/${journey}${at}?view=${view}${query === undefined ? "" : `&${query}`}`;
}

/**
 * Holds the page on the day `day`: its clock at noon UTC, and the documents the server sends
 * derived for that day (the server reads its own clock, which the fixtures' dates are not
 * written for). Call it before the page loads.
 */
export async function holdToday(page: Page, day: string): Promise<void> {
  await page.clock.setFixedTime(new Date(`${day}T12:00:00Z`));
  await page.route("**/api/journeys/*/document", async (route) => {
    const response = await route.fetch();
    const document = (await response.json()) as { inputs?: { today: string } };
    if (document.inputs === undefined) {
      await route.fulfill({ response, json: document });
      return;
    }
    document.inputs.today = day;
    await route.fulfill({ response, json: document });
  });
}

/**
 * Opens journey `journey`'s page at `address` (`list?frontier=1`, `timeline`, `summary`, with a
 * query), with node `node`'s detail beside it when given, once derived. With `fixedToday` the
 * page is held on that day first (holdToday), so the journey derives with that today.
 */
export async function openAt(page: Page, journey: string, address: string, options: { node?: string; fixedToday?: string } = {}): Promise<void> {
  if (options.fixedToday !== undefined) {
    await holdToday(page, options.fixedToday);
  }
  await open(page, journeyAddress(journey, address, options.node));
  await expect(page.getByTestId("journey-frame")).toBeVisible();
  await derived(page);
  if (options.fixedToday !== undefined) {
    await expect(syncChip(page)).toHaveAttribute("data-today", options.fixedToday);
  }
}

/** Opens the toolbar's filter, unless it is open. */
export async function openFilter(page: Page): Promise<void> {
  if (!(await whenSettled(page, () => page.getByTestId("filter-panel").isVisible()))) {
    await page.getByTestId("filter-button").click();
  }
  await expect(page.getByTestId("filter-panel")).toBeVisible();
}

/** Picks the depth `word` (Stages, Decisions, Deliverables, Actions) from the view controls' Depth menu. */
export async function setDepth(page: Page, word: string): Promise<void> {
  await page.getByTestId("depth-button").click();
  await page.getByTestId("ladder").getByText(word, { exact: true }).click();
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName(`Depth: ${word}`);
}

/** Ticks or unticks the Depth menu's Fade checkbox, then closes the menu (it stays open for the checkbox). */
export async function setFade(page: Page, on: boolean): Promise<void> {
  await page.getByTestId("depth-button").click();
  const check = page.getByTestId("depth-fade").getByRole("checkbox");
  if ((await whenSettled(page, () => check.isChecked())) !== on) {
    await check.click();
  }
  // The new state comes back from the address.
  await expect(check).toBeChecked({ checked: on });
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("depth-fade")).toHaveCount(0);
}

/** Goes to the view `view` (`graph`, `list`, `timeline`, `summary` or `walk`) from the header's View picker, as a person does. */
export async function goTo(page: Page, view: string): Promise<void> {
  const frame = page.getByTestId("journey-frame");
  if ((await whenSettled(page, () => frame.getAttribute("data-screen"))) !== view) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${view}`).click();
  }
  await expect(frame).toHaveAttribute("data-screen", view);
}

/** A window wide enough that a fitted graph is at the reading zoom, so every card is a card and not inside a tile. */
export async function readable(page: Page): Promise<void> {
  await page.setViewportSize({ width: 3200, height: 2000 });
}

/** Node `node`'s card on the journey's canvas. */
export function nodeCard(page: Page, node: string) {
  return page.locator(`[data-testid="node-card"][data-node="${node}"]`);
}

/** Node `node`'s detail panel. */
export function nodePanel(page: Page, node: string) {
  return page.locator(`[data-testid="node-detail"][data-node="${node}"]`);
}

/**
 * Draws `node`'s card on the page. A card inside a container that the fit draws as a tile is not on
 * the page: the app opens it at its address (no reload, so the host keeps its data) and the canvas
 * frames it, which draws it as itself.
 */
export async function reachCard(page: Page, node: string): Promise<void> {
  const card = nodeCard(page, node);
  const open = card.getByTestId("card-open");
  const canvas = page.getByTestId("canvas");
  // On the canvas, not beside it (a card panned under the rail is drawn but cannot be clicked).
  const onCanvas = () =>
    whenSettled(page, async () => {
      if ((await open.count()) === 0) {
        return false;
      }
      const [at, within] = await Promise.all([open.boundingBox(), canvas.boundingBox()]);
      return at !== null && within !== null && at.x >= within.x && at.y >= within.y && at.x + at.width <= within.x + within.width && at.y + at.height <= within.y + within.height;
    });
  if (await onCanvas()) {
    return;
  }
  const { pathname, search } = new URL(page.url());
  await follow(page, `${pathname.replace(/\/(nodes|edges)\/[^/]*$/, "").replace(/\/$/, "")}/nodes/${node}${search}`);
  await page.getByRole("button", { name: /^Frame the selected card/ }).click();
  await expect.poll(onCanvas).toBe(true);
}

/** Opens `node`'s detail from its card on the canvas, unless it is open already. */
export async function openFromCanvas(page: Page, node: string) {
  const panel = nodePanel(page, node);
  if (!(await whenSettled(page, () => panel.isVisible()))) {
    await reachCard(page, node);
    if (!(await whenSettled(page, () => panel.isVisible()))) {
      await nodeCard(page, node).getByTestId("card-open").click();
    }
  }
  await expect(panel).toBeVisible();
  return panel;
}

/** Chooses `item` (`rename`, `reopen`, `done-anyway`, ...) from the inspector's overflow menu. */
export async function menuItem(panel: Locator, item: string): Promise<void> {
  await panel.getByTestId("more-actions").click();
  await panel.page().getByTestId(`menu-${item}`).click();
}

/** The title editor in `node`'s detail panel. */
export function renameOf(page: Page, node: string) {
  return nodePanel(page, node).getByTestId("rename");
}

/** Starts renaming `node` from its detail and types `text`, without saving. */
export async function startRename(page: Page, node: string, text: string): Promise<void> {
  await openFromCanvas(page, node);
  await menuItem(nodePanel(page, node), "rename");
  await renameOf(page, node).getByRole("textbox").fill(text);
}

export async function save(page: Page, node: string): Promise<void> {
  await renameOf(page, node).getByRole("button", { name: "Save" }).click();
}

/** Renames `node` to `text` and waits for its card to show it and the page to settle. */
export async function rename(page: Page, node: string, text: string): Promise<void> {
  await startRename(page, node, text);
  await save(page, node);
  await expect(nodeCard(page, node).getByTestId("title")).toHaveText(text);
  await settled(page);
}

/** The revision the page's derivation is at, once it has settled. */
export async function derivedRevision(page: Page): Promise<number> {
  return Number(await whenSettled(page, () => syncChip(page).getAttribute("data-revision")));
}

/** A title no earlier run used. */
export function fresh(stem: string): string {
  return `${stem} ${Math.random().toString(36).slice(2, 8)}`;
}

/** Counts the page's fetches of `journey`'s document. */
export function countDocumentFetches(page: Page, journey: string): () => number {
  let count = 0;
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === `/api/journeys/${journey}/document`) {
      count += 1;
    }
  });
  return () => count;
}

/** On a phone the inspector opens at its peek, which shows its tabs and not its forms: opens it to full first, where it is a sheet at its peek. */
export async function openSheet(page: Page): Promise<void> {
  const handle = page.getByRole("button", { name: "Resize the inspector" });
  // The frame's own layout, drawn with what the caller waited for, not data: a test holding a read open opens the sheet too.
  // eslint-disable-next-line e2e/settled-reads
  if ((await handle.isVisible()) && (await page.locator(".frame").getAttribute("data-sheet")) === "peek") {
    await handle.click();
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "full");
  }
}

/**
 * A journey only this test touches, on the server host: started through one API patch from
 * version 1 of `route`, under a name no other run used. The server tests that run side by side
 * each make their own, so none reads or writes another's (the fixtures' journeys stay as seeded).
 * A journey is created against the deployment revision it was checked under, so a refusal as
 * stale (another test moved the deployment meanwhile) is asked again.
 */
export async function ownJourney(request: APIRequestContext, route: string, stem = "Own"): Promise<{ id: string; name: string }> {
  const name = fresh(stem);
  const id = `j_${name.replace(/\W/g, "_").toLowerCase()}`;
  for (let attempt = 0; ; attempt += 1) {
    const response = await request.post(`/api/journeys/${id}/patches`, {
      data: {
        patch: {
          id: `p_${crypto.randomUUID().replaceAll("-", "")}`,
          target: { journey: id },
          base_revision: 0,
          mutations: [{ op: "create_journey", name, from: { route, version: 1 } }],
        },
      },
    });
    const body = await response.text();
    if (response.ok()) {
      return { id, name };
    }
    expect(body, `starting ${id}`).toContain("stale");
    expect(attempt, `starting ${id} kept going stale`).toBeLessThan(5);
  }
}
