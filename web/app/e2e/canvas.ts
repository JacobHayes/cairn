// What the canvas's browser tests share: the cards and lines a canvas draws, read the way the
// page shows them, the toggles, and where the layout put each card.
import { expect, type Page } from "@playwright/test";

import { closeFilter, openFilter } from "./shell.ts";

/** The keys of the cards on the canvas, sorted. */
export async function cardKeys(page: Page): Promise<string[]> {
  const keys = await page.getByTestId("node-card").evaluateAll((cards) => cards.map((card) => card.getAttribute("data-node") ?? ""));
  return keys.sort();
}

/** Each card's container, by key ("top" at the top level). */
export async function containers(page: Page): Promise<Record<string, string>> {
  const pairs = await page
    .getByTestId("node-card")
    .evaluateAll((cards) => cards.map((card) => [card.getAttribute("data-node") ?? "", card.getAttribute("data-parent") ?? "top"]));
  return Object.fromEntries(pairs) as Record<string, string>;
}

/** C15: where the layout put each card in its container, by key. */
export async function places(page: Page): Promise<Record<string, { x: number; y: number; parent: string }>> {
  const rows = await page.getByTestId("node-card").evaluateAll((cards) =>
    cards.map((card) => [
      card.getAttribute("data-node") ?? "",
      { x: Number(card.getAttribute("data-x")), y: Number(card.getAttribute("data-y")), parent: card.getAttribute("data-parent") ?? "top" },
    ]),
  );
  return Object.fromEntries(rows) as Record<string, { x: number; y: number; parent: string }>;
}

/** Each card's mark from the overlay (C7's trace), by key; unmarked cards are left out. */
export async function marks(page: Page): Promise<Record<string, string>> {
  const pairs = await page
    .locator("[data-testid=node-card][data-trace]")
    .evaluateAll((cards) => cards.map((card) => [card.getAttribute("data-node") ?? "", card.getAttribute("data-trace") ?? ""]));
  return Object.fromEntries(pairs) as Record<string, string>;
}

/**
 * Turns the canvas toggle `name` (a kind, `not-relevant`, `undecided`, `heat`) on or off and
 * waits for it to hold: a toggle changes the address, and the box follows it.
 */
export async function toggle(page: Page, name: string, on: boolean): Promise<void> {
  // A journey's toggles are in the toolbar's filter; a route's are on its canvas page.
  const filtered = (await page.getByTestId("filter-button").count()) > 0;
  if (filtered) {
    await openFilter(page);
  }
  const control = page.getByTestId(`show-${name}`);
  if ((await control.getAttribute("data-status")) !== (on ? "on" : "off")) {
    await control.locator("input").click();
  }
  await expect(control).toHaveAttribute("data-status", on ? "on" : "off");
  if (filtered) {
    await closeFilter(page);
  }
}

/** Shows or hides one kind with its toggle (C2). */
export function showKind(page: Page, kind: string, shown: boolean): Promise<void> {
  return toggle(page, kind, shown);
}

/** A card's line on the canvas, from `from` to `to`. */
export function lineBetween(page: Page, from: string, to: string) {
  return page.locator(`[data-testid=edge-line][data-from="${from}"][data-to="${to}"]`);
}
