// What the canvas's browser tests share: the lines a canvas draws, read the way the
// page shows them, and its viewport.
import { expect, type Locator, type Page } from "@playwright/test";

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

/** Waits for the viewport to stop moving (a layout change refits it with an animation). */
export async function viewportSettled(page: Page): Promise<void> {
  let last = "";
  await expect
    .poll(async () => {
      const now = JSON.stringify(await viewportOf(page));
      const still = now === last;
      last = now;
      return still;
    })
    .toBe(true);
}

/**
 * Every card's title wraps to the lines layout counted for it (`--title-lines`, from the title's
 * measured width, cards.ts): not a line fewer (cut with an ellipsis for nothing) and not a blank
 * one more (a card taller than its text). The title's lines are read from its text's own boxes,
 * which a clamp does not hide. An open container is left out: its width is ELK's, around its
 * children, so the header's lines are counted for the usual card width and its title may take fewer.
 */
export async function expectTitleLinesAsLaidOut(page: Page): Promise<void> {
  const off = await page.evaluate(() => {
    const wrong: string[] = [];
    let checked = 0;
    for (const card of document.querySelectorAll<HTMLElement>('[data-testid="node-card"]')) {
      const title = card.querySelector<HTMLElement>('[data-testid="title"]');
      const row = card.querySelector<HTMLElement>(".node-title-row");
      if (title === null || row === null || card.classList.contains("node-container")) {
        continue;
      }
      const range = document.createRange();
      range.selectNodeContents(title);
      const natural = new Set([...range.getClientRects()].map((line) => Math.round(line.top / 2))).size;
      const laid = Number(row.style.getPropertyValue("--title-lines"));
      checked += 1;
      if (Math.min(natural, 2) !== laid) {
        wrong.push(`${card.dataset["node"] ?? ""}: laid out as ${String(laid)} lines, drawn as ${String(natural)}`);
      }
    }
    return { checked, wrong };
  });
  expect(off.checked, "titles looked at").toBeGreaterThan(5);
  expect(off.wrong).toEqual([]);
}
