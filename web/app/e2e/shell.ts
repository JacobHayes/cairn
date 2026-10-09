// What the app's browser tests share: opening a screen on a host, and reading and editing a
// journey page the way a person does: a node's card on the canvas, its detail beside it.
import { expect, type Locator, type Page } from "@playwright/test";

export type HostKind = "server" | "browser";

/** The in-browser host's dev server (playwright.config.ts); the server host is the project's own. */
const DEMO = `http://127.0.0.1:${process.env["CAIRN_DEMO_PORT"] ?? ""}`;

/**
 * Opens the screen at `path` (`/`, `/journeys/<id>?view=...`) on `host`, once the shell is up:
 * within the page when it already runs on `host`, as a link would, so the in-browser host's
 * store (which lives only as long as the page) is kept; by loading the page otherwise. The
 * sync chip says which host it is on (`data-host`).
 */
export async function open(page: Page, host: HostKind, path = "/"): Promise<void> {
  const nav = page.getByRole("navigation", { name: "Screens", exact: true });
  const onDemo = page.url().startsWith(`${DEMO}/`);
  if ((await nav.count()) === 1 && onDemo === (host === "browser")) {
    await goWithin(page, path);
  } else {
    await page.goto(host === "browser" ? `${DEMO}${path}` : path);
  }
  await expect(nav).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-host", host);
}

/** The sync chip at the strip's right end. */
export function syncChip(page: Page): Locator {
  return page.getByTestId("sync");
}

/** Waits until the screen's journey is derived: its name is up, and the chip knows its revision. */
export async function derived(page: Page): Promise<void> {
  await expect(page.getByTestId("journey-name")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("data-revision", /^\d+$/);
}

/** Waits until the stream is live and nothing is pending: the chip says "In sync, live". */
export async function live(page: Page): Promise<void> {
  await expect(syncChip(page)).toHaveAttribute("title", /^In sync · live/);
}

/** The popover's Recent, newest first, once it lists a save; the popover is closed again. */
export async function recentSaves(page: Page): Promise<string[]> {
  const chip = syncChip(page);
  if ((await chip.getAttribute("aria-expanded")) !== "true") {
    await chip.click();
  }
  const recent = page.getByTestId("sync-popover").getByTestId("sync-recent");
  await expect(recent.first()).toBeVisible();
  const saves = await recent.allTextContents();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sync-popover")).toHaveCount(0);
  return saves;
}

/** Loads the screen at `path` afresh on `host`, as a bookmark or a posted link does, and returns once the shell is up. */
export async function visit(page: Page, host: HostKind, path: string): Promise<void> {
  await page.goto(host === "browser" ? `${DEMO}${path}` : path);
  await expect(page.getByRole("navigation", { name: "Screens", exact: true })).toBeVisible();
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
 * and returns once the router has rendered it (the shell's `data-screen`). Data the screen
 * derives may still be on its way; read it with a web-first assertion.
 */
export async function goWithin(page: Page, path: string): Promise<void> {
  await follow(page, path);
  const main = page.locator("main[data-screen]");
  await expect.poll(async () => screenOf((await main.getAttribute("data-screen")) ?? "")).toBe(screenOf(path));
}

/** The screen an address names: its path and its query, in a stable order. */
function screenOf(address: string): string {
  const url = new URL(address, "http://screen.invalid");
  url.searchParams.sort();
  return `${url.pathname}?${url.searchParams.toString()}`;
}

/**
 * A journey's page, PLAN, GRAPH, once derived and its canvas drawn. The graph opens at its All
 * step unless `query` says otherwise, so every node has a card to reach (it opens at Stages, with
 * most stages folded, for a person).
 */
export async function openJourney(page: Page, host: HostKind, journey: string, query = "?detail=all"): Promise<void> {
  await open(page, host, `/journeys/${journey}/plan/graph${query}`);
  await derived(page);
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  // A spec reaches any node, so it sees the whole graph; the real default opens on the current stage (the canvas flow test).
  if (query !== "") {
    // After the opening fit has moved the viewport off its start, so it cannot land after this one.
    await page.waitForFunction(() => document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform !== "translate(0px, 0px) scale(1)");
    await page.keyboard.press("f");
  }
}

/**
 * Opens journey `journey`'s page at `address` (`next/list`, `plan/timeline`, `summary`, with a
 * query), with node `node`'s detail beside it when given, once derived. On the browser host the
 * page's clock is held at noon UTC on `fixedToday` first, so the host derives with that today.
 */
export async function openAt(page: Page, host: HostKind, journey: string, address: string, options: { node?: string; fixedToday?: string } = {}): Promise<void> {
  if (host === "browser" && options.fixedToday !== undefined) {
    await page.clock.setFixedTime(new Date(`${options.fixedToday}T12:00:00Z`));
  }
  const [path = "", query] = address.split("?");
  await open(page, host, `/journeys/${journey}/${path}${options.node === undefined ? "" : `/nodes/${options.node}`}${query === undefined ? "" : `?${query}`}`);
  await expect(page.getByTestId("journey-frame")).toBeVisible();
  await derived(page);
  if (host === "browser" && options.fixedToday !== undefined) {
    await expect(syncChip(page)).toHaveAttribute("data-today", options.fixedToday);
  }
}

/** Opens the toolbar's filter, unless it is open. */
export async function openFilter(page: Page): Promise<void> {
  if (!(await page.getByTestId("filter-panel").isVisible())) {
    await page.getByTestId("filter-button").click();
  }
  await expect(page.getByTestId("filter-panel")).toBeVisible();
}

/** Closes the toolbar's filter. */
export async function closeFilter(page: Page): Promise<void> {
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("filter-panel")).toHaveCount(0);
}

/** Goes to `page`'s tab (`next` or `plan`) and its projection, as a person does. */
export async function goTo(page: Page, tab: "next" | "plan", projection: string): Promise<void> {
  await page.getByTestId(`tab-${tab}`).click();
  // The switcher has a List on both pages: wait for the page, or the other page's is read.
  await expect(page.getByTestId(`tab-${tab}`)).toHaveAttribute("aria-current", "page");
  const switcher = page.getByTestId(`projection-${projection}`);
  if ((await switcher.getAttribute("aria-current")) !== "page") {
    await switcher.click();
  }
  await expect(switcher).toHaveAttribute("aria-current", "page");
}

/** Node `node`'s card on the journey's canvas. */
export function nodeCard(page: Page, node: string) {
  return page.locator(`[data-testid="node-card"][data-node="${node}"]`);
}

/** Node `node`'s detail panel. */
export function nodePanel(page: Page, node: string) {
  return page.locator(`[data-testid="node-detail"][data-node="${node}"]`);
}

/** Opens `node`'s detail from its card on the canvas, unless it is open already. */
export async function openFromCanvas(page: Page, node: string) {
  const panel = nodePanel(page, node);
  if (!(await panel.isVisible())) {
    await nodeCard(page, node).getByTestId("card-open").click();
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

/** Renames `node` to `text` and waits for its card to show it. */
export async function rename(page: Page, node: string, text: string): Promise<void> {
  await startRename(page, node, text);
  await save(page, node);
  await expect(nodeCard(page, node).getByTestId("title")).toHaveText(text);
}

/** The revision the page's derivation is at. */
export async function derivedRevision(page: Page): Promise<number> {
  return Number(await syncChip(page).getAttribute("data-revision"));
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

/** Holds every request matching `url` on `page` until `release` is called, then lets it through. */
export async function hold(page: Page, url: string): Promise<() => Promise<void>> {
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(url, async (route) => {
    await held;
    await route.continue().catch(() => undefined);
  });
  return async () => {
    release();
    await page.unroute(url);
  };
}
