// What the canvas's browser tests share: the lines a canvas draws, read the way the
// page shows them, and its viewport.
import { readFileSync } from "node:fs";

import { expect, type Locator, type Page } from "@playwright/test";

import { fixtureRoute, publishDraft, versionSelect } from "./around.ts";
import { importRoute } from "./shell.ts";

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

/** Titles of the lengths that make a card's title wrap: one of wide capitals that still fits the character count of a line, a long one of ordinary words, and a short one. */
const LONG_TITLES = ["MAKE ALL WORK MATCH WELL", "Settle the long list of small items that stand between the two stages", "Short title"];

/**
 * Publishes, on the in-browser host, a route like the vendor evaluation whose nodes carry titles
 * that wrap to one, two and more lines, and returns its id.
 */
export async function publishLongTitleRoute(page: Page): Promise<string> {
  const route = "long-titles";
  const text = readFileSync(fixtureRoute("vendor-evaluation"), "utf8");
  const at = text.indexOf("\nnodes:");
  let node = 0;
  const nodes = text.slice(at).replace(/^( {2}| {4}|(?: {2}){3})title: .*$/gm, (_all, indent: string) => `${indent}title: ${LONG_TITLES[node++ % LONG_TITLES.length] ?? ""}`);
  const file = (text.slice(0, at) + nodes).replace("route: vendor-evaluation\nname: Vendor evaluation\n", `route: ${route}\nname: Long titles\n`);
  await importRoute(page, "browser", { name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(file) });
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
  return route;
}

/** Zooms the canvas in with its button until it is at least at the reading zoom (1). */
export async function zoomToReading(page: Page): Promise<void> {
  for (let step = 0; step < 12 && (await viewportOf(page)).zoom < 1; step += 1) {
    await page.getByRole("button", { name: /zoom in/i }).click();
    await viewportSettled(page);
  }
  expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(1);
}

/**
 * Every card's rows lie inside the card's box and below the row above (in the card's own px, so
 * the zoom does not matter): none is cut by the border or printed over the next. A row a zoomed-out
 * container head hides has no box and is left out. The cards are
 * counted so a canvas that drew none does not pass.
 */
export async function expectRowsInsideCards(page: Page): Promise<void> {
  const problems = await page.evaluate(() => {
    const found: string[] = [];
    // A title's lines that a clip (the clamp, a row's box) hides are not drawn, so only what shows counts; any other row is its own box.
    const shown = (row: HTMLElement, card: HTMLElement): { top: number; bottom: number; right: number; height: number } => {
      if (row.dataset["testid"] !== "title") {
        return row.getBoundingClientRect();
      }
      const clips: DOMRect[] = [];
      for (let up: HTMLElement | null = row; up !== null && up !== card; up = up.parentElement) {
        if (getComputedStyle(up).overflow !== "visible") {
          clips.push(up.getBoundingClientRect());
        }
      }
      const range = document.createRange();
      range.selectNodeContents(row);
      // A text rect is the glyphs' box, taller than the line: the line box is the computed line height centred on it.
      const scale = card.getBoundingClientRect().height / card.offsetHeight;
      const half = (Number.parseFloat(getComputedStyle(row).lineHeight) * scale) / 2;
      const lines = [...range.getClientRects()]
        .map((line) => ({ top: (line.top + line.bottom) / 2 - half, bottom: (line.top + line.bottom) / 2 + half, right: line.right }))
        .filter((line) => clips.every((clip) => line.top < clip.bottom - 0.5 && line.bottom > clip.top));
      const [top, bottom, right] = [Math.min(...lines.map((line) => line.top)), Math.max(...lines.map((line) => Math.min(line.bottom, ...clips.map((clip) => clip.bottom)))), Math.max(...lines.map((line) => line.right))];
      return lines.length === 0 ? { top: 0, bottom: 0, right: 0, height: 0 } : { top, bottom, right, height: bottom - top };
    };
    const cards = [...document.querySelectorAll<HTMLElement>('[data-testid="node-card"]')];
    for (const card of cards) {
      const box = card.getBoundingClientRect();
      const scale = box.height / card.offsetHeight;
      const rows = [...card.querySelectorAll<HTMLElement>(".node-head, .node-title-row, [data-testid=\"title\"], .node-body-line, .node-foot")].map((row) => ({ row, at: shown(row, card) })).filter(({ at }) => at.height > 0);
      for (const { row, at } of rows) {
        // The border is 1px: a row ends above it.
        if (at.bottom > box.bottom - scale + 0.5 * scale || at.top < box.top || at.right > box.right + 0.5 * scale) {
          found.push(`${card.dataset["node"] ?? ""}: ${row.dataset["testid"] ?? row.className} sticks out of the card (${String(Math.round(at.top - box.top))}..${String(Math.round(at.bottom - box.top))} of ${String(Math.round(box.height))}, right ${String(Math.round(at.right - box.right))})`);
        }
      }
      for (const [index, { row, at }] of rows.entries()) {
        const next = rows.slice(index + 1).find((other) => !row.contains(other.row) && !other.row.contains(row) && other.at.top >= at.top - 0.5 * scale);
        if (next !== undefined && at.bottom > next.at.top + 0.5 * scale) {
          found.push(`${card.dataset["node"] ?? ""}: ${row.dataset["testid"] ?? row.className} runs over ${next.row.dataset["testid"] ?? next.row.className}`);
        }
      }
    }
    return [`${String(cards.length)} cards`, ...found];
  });
  expect(Number.parseInt(problems[0] ?? "0", 10)).toBeGreaterThan(5);
  expect(problems.slice(1)).toEqual([]);
}
