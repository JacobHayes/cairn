// What the app's browser tests share: opening a screen on a host, and reading and editing a
// journey page the way a person does.
import { expect, type Page } from "@playwright/test";

export type HostKind = "server" | "browser";

/** Opens the screen at `hash` (`/`, `/journeys/<id>`) on `host`, once the shell is up. */
export async function open(page: Page, host: HostKind, hash = "/"): Promise<void> {
  await page.goto(`/?host=${host}#${hash}`);
  await expect(page.getByTestId("host")).toBeVisible();
}

/** A journey's page, once derived. */
export async function openJourney(page: Page, host: HostKind, journey: string): Promise<void> {
  await open(page, host, `/journeys/${journey}`);
  await expect(page.getByTestId("derivation")).toBeVisible();
}

export function nodeRow(page: Page, node: string) {
  return page.locator(`[data-testid="node-row"][data-node="${node}"]`);
}

/** Starts renaming `node` and types `text`, without saving. */
export async function startRename(page: Page, node: string, text: string): Promise<void> {
  const row = nodeRow(page, node);
  await row.getByRole("button", { name: /^Rename/ }).click();
  await row.getByRole("textbox").fill(text);
}

export async function save(page: Page, node: string): Promise<void> {
  await nodeRow(page, node).getByRole("button", { name: "Save" }).click();
}

/** Renames `node` to `text` and waits for the page to show it. */
export async function rename(page: Page, node: string, text: string): Promise<void> {
  await startRename(page, node, text);
  await save(page, node);
  await expect(nodeRow(page, node).getByTestId("title")).toHaveText(text);
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
    if (new URL(request.url()).pathname === `/journeys/${journey}/document`) {
      count += 1;
    }
  });
  return () => count;
}
