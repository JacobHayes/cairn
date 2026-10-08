// What the app's browser tests share: opening a screen on a host, and reading and editing a
// journey page the way a person does: a node's card on the canvas, its detail beside it.
import { expect, type Page } from "@playwright/test";

export type HostKind = "server" | "browser";

/**
 * Opens the screen at `path` (`/`, `/journeys/<id>?view=...`) on `host`, once the shell is up:
 * within the page when it already runs on `host`, as a link would, so the in-browser host's
 * store (which lives only as long as the page) is kept; by loading the page otherwise.
 */
export async function open(page: Page, host: HostKind, path = "/"): Promise<void> {
  const shell = page.getByTestId("host");
  if ((await shell.count()) === 1 && (await shell.getAttribute("data-status")) === host) {
    await goWithin(page, path);
  } else {
    await page.goto(`${path}${path.includes("?") ? "&" : "?"}host=${host}`);
  }
  await expect(shell).toBeVisible();
}

/** Moves the tab to the screen at `path` without loading the page again, as the app's links do. */
export async function goWithin(page: Page, path: string): Promise<void> {
  await page.evaluate((to) => {
    history.pushState(null, "", to);
    dispatchEvent(new PopStateEvent("popstate"));
  }, path);
}

/** A journey's page, once derived and its canvas drawn. */
export async function openJourney(page: Page, host: HostKind, journey: string, query = ""): Promise<void> {
  await open(page, host, `/journeys/${journey}${query}`);
  await expect(page.getByTestId("derivation")).toBeVisible();
  await expect(page.getByTestId("node-card").first()).toBeVisible();
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

/** The title editor in `node`'s detail panel. */
export function renameOf(page: Page, node: string) {
  return nodePanel(page, node).getByTestId("rename");
}

/** Starts renaming `node` from its detail and types `text`, without saving. */
export async function startRename(page: Page, node: string, text: string): Promise<void> {
  await openFromCanvas(page, node);
  const editor = renameOf(page, node);
  await editor.getByRole("button", { name: /^Rename/ }).click();
  await editor.getByRole("textbox").fill(text);
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
  return Number(await page.getByTestId("derivation").getAttribute("data-revision"));
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
