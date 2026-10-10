// What the canvas's browser tests and proofs share: the lines a canvas draws, read the way the
// page shows them, and its viewport.
import type { Locator, Page } from "@playwright/test";

/** A card's line on the canvas, from `from` to `to`. */
export function lineBetween(page: Page, from: string, to: string) {
  return page.locator(`[data-testid=edge-line][data-from="${from}"][data-to="${to}"]`);
}

/** A point on the line, in the page's pixels: where a pointer hovers or clicks it. */
export function pointOn(line: Locator): Promise<{ x: number; y: number }> {
  return line.locator("path").first().evaluate((path: SVGPathElement) => {
    const [at, m] = [path.getPointAtLength(path.getTotalLength() / 2), path.getScreenCTM()];
    return { x: at.x * (m?.a ?? 1) + at.y * (m?.c ?? 0) + (m?.e ?? 0), y: at.x * (m?.b ?? 0) + at.y * (m?.d ?? 1) + (m?.f ?? 0) };
  });
}

/** The canvas's viewport: its offset and zoom. */
export async function viewportOf(page: Page): Promise<{ x: number; y: number; zoom: number }> {
  const style = (await page.locator(".react-flow__viewport").getAttribute("style")) ?? "";
  const [x = 0, y = 0] = /translate\(([-\d.]+)px, ([-\d.]+)px\)/.exec(style)?.slice(1).map(Number) ?? [];
  return { x, y, zoom: Number(/scale\(([\d.]+)\)/.exec(style)?.[1] ?? 1) };
}
