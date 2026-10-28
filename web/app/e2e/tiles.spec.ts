// A container zoomed out is a tile: the real zoom on the large program, whose fit is far below the
// reading zoom. Nothing moves when a stage becomes a tile, what it holds is not drawn, lines run
// to the tile, and what the canvas does with a card (selection, search, focus, a double-click) reads
// the one mapping from a card to the tile that stands for it.
import type { Page } from "@playwright/test";

import { lineBetween, viewportOf } from "./canvas.ts";
import { LARGE, openLargeGraph, startLargeProgram } from "./large-program.ts";
import { expect, test } from "./test.ts";
import { nodeCard, open, whenSettled } from "./shell.ts";
import { search } from "./view-state.ts";

/** The reading zoom: at or above it nothing is a tile. */
const READING = 0.6;

/** Zooms the canvas with a pinch at its middle: `steps` notches in (negative: out). */
async function pinch(page: Page, steps: number): Promise<void> {
  const box = await whenSettled(page, () => page.getByTestId("canvas").boundingBox());
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  for (let step = 0; step < Math.abs(steps); step += 1) {
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, steps > 0 ? -120 : 120);
    await page.keyboard.up("Control");
  }
}

/** Zooms in until the canvas is at least at `zoom`. */
async function zoomTo(page: Page, zoom: number): Promise<void> {
  await expect
    .poll(async () => {
      const now = (await viewportOf(page)).zoom;
      if (now < zoom) {
        await pinch(page, 1);
      }
      return (await viewportOf(page)).zoom >= zoom;
    })
    .toBe(true);
}

/** The large program's graph, fitted: stages are tiles. */
async function fitted(page: Page): Promise<string> {
  await page.setViewportSize({ width: 1280, height: 800 });
  const id = await startLargeProgram(page, "browser");
  await openLargeGraph(page, "browser", id);
  await page.keyboard.press("f");
  await expect.poll(async () => (await viewportOf(page)).zoom).toBeLessThan(0.2);
  return id;
}

const tile = (page: Page, key: string) => page.locator(`[data-testid="node-card"][data-node="${key}"][data-tile]`);

test("a stage is drawn as a tile at the fit, nothing inside it is drawn, and it opens in place without moving", async ({ page }) => {
  await fitted(page);
  for (const stage of LARGE.stages) {
    await expect(tile(page, stage)).toBeVisible();
  }
  // No card inside a stage is drawn: only tiles and the cards outside every container.
  await expect(page.locator('[data-testid="node-card"][data-parent]')).toHaveCount(0);
  const [x, y] = await whenSettled(page, async () => [await nodeCard(page, LARGE.build).getAttribute("data-x"), await nodeCard(page, LARGE.build).getAttribute("data-y")]);
  await zoomTo(page, READING);
  await expect(tile(page, LARGE.build)).toHaveCount(0);
  await expect(nodeCard(page, LARGE.build)).toHaveAttribute("data-x", x ?? "");
  await expect(nodeCard(page, LARGE.build)).toHaveAttribute("data-y", y ?? "");
  // At the reading zoom nothing is a tile.
  await expect(page.locator("[data-tile]")).toHaveCount(0);
});

test("a line between two tiles is drawn once, ends on both, and its card lists every link it stands for", async ({ page }) => {
  const id = await fitted(page);
  await open(page, "browser", `/journeys/${id}/nodes/${LARGE.helpers}?view=graph&depth=actions`);
  await zoomTo(page, READING);
  // Zoom out until the groups of the build are tiles.
  await expect
    .poll(async () => {
      if ((await tile(page, LARGE.partA).count()) === 0) {
        await pinch(page, -1);
      }
      return tile(page, LARGE.partA).count();
    })
    .toBe(1);
  const between = lineBetween(page, LARGE.partA, LARGE.helpers);
  await expect(between).toHaveCount(1);
  // The line may lie under the inspector at this zoom, so it is clicked where it is, not at a point.
  await between.locator("path").first().dispatchEvent("click");
  const card = page.getByTestId("edge-card");
  await expect(card).toHaveAttribute("data-from", LARGE.partA);
  await expect(card).toHaveAttribute("data-to", LARGE.helpers);
  // Two links run from the first group to the second; one line stands for both.
  await expect(card.getByTestId("edge-links").locator("li")).toHaveCount(2);
  // The line's ends are on the two tiles (the arrowhead holds the end a few pixels off), not on the cards they hold.
  const [from, to] = await whenSettled(page, () => Promise.all([tile(page, LARGE.partA).boundingBox(), tile(page, LARGE.helpers).boundingBox()]));
  const ends = await whenSettled(page, () => between.locator("path").first().evaluate((path: SVGPathElement) => {
    const matrix = path.getScreenCTM();
    const [first, last] = [path.getPointAtLength(0), path.getPointAtLength(path.getTotalLength())];
    return matrix === null ? [] : [first, last].map((point) => ({ x: point.x * matrix.a + matrix.e, y: point.y * matrix.d + matrix.f }));
  }));
  const near = (point: { x: number; y: number } | undefined, box: { x: number; y: number; width: number; height: number } | null) =>
    point !== undefined && box !== null && point.x >= box.x - 12 && point.x <= box.x + box.width + 12 && point.y >= box.y - 12 && point.y <= box.y + box.height + 12;
  expect(near(ends[0], from)).toBe(true);
  expect(near(ends[1], to)).toBe(true);
});

test("a selection inside a tile is named on it, and a search says how many matches the tile holds", async ({ page }) => {
  const id = await fitted(page);
  await open(page, "browser", `/journeys/${id}/nodes/${LARGE.componentTwo}?view=graph&depth=actions`);
  await zoomTo(page, READING);
  await pinch(page, -8);
  await expect.poll(async () => (await viewportOf(page)).zoom).toBeLessThan(0.3);
  const stood = page.locator("[data-tile]").filter({ has: page.getByTestId("tile-fact") });
  await expect(stood.getByTestId("tile-fact")).toHaveText("Component two selected");
  await page.keyboard.press("Escape");
  await search(page, "component");
  await page.keyboard.press("f");
  const holder = page.locator("[data-tile]").filter({ has: page.getByTestId("tile-fact") }).first();
  const said = Number(/(\d+) match/.exec((await whenSettled(page, () => holder.getByTestId("tile-fact").textContent())) ?? "")?.[1]);
  const stage = (await whenSettled(page, () => holder.getAttribute("data-node"))) ?? "";
  expect(said).toBeGreaterThan(1);
  // Zoomed in, the stage holds exactly that many nodes whose title says "component" (containers among them), and the tile counted each.
  await zoomTo(page, READING);
  const inside = await whenSettled(page, () =>
    page.getByTestId("node-card").evaluateAll(
      (cards, outer) => {
        const parents = new Map(cards.map((card) => [(card as HTMLElement).dataset["node"], (card as HTMLElement).dataset["parent"]]));
        const held = (key: string | undefined): boolean => (key === undefined ? false : key === outer || held(parents.get(key)));
        return cards.filter((card) => (card as HTMLElement).dataset["node"] !== outer && held((card as HTMLElement).dataset["node"]) && /component/i.test(card.querySelector('[data-testid="title"]')?.textContent ?? "")).length;
      },
      stage,
    ),
  );
  expect(inside).toBe(said);
});

test("focus on a card that a tile forms over moves to the tile, and a double-click on a tile frames it drawn open", async ({ page }) => {
  const id = await fitted(page);
  await open(page, "browser", `/journeys/${id}/nodes/${LARGE.partA}?view=graph&depth=actions`);
  await zoomTo(page, READING);
  await page.locator(`.react-flow__node[data-id="${LARGE.partA}"]`).focus();
  await pinch(page, -8);
  await expect.poll(async () => (await viewportOf(page)).zoom).toBeLessThan(0.2);
  await expect(page.locator("[data-tile]").first()).toBeVisible();
  await expect.poll(() => page.evaluate(() => (document.activeElement as HTMLElement | null)?.closest(".react-flow__node")?.getAttribute("data-id") ?? "")).toBe(LARGE.build);
  await page.keyboard.press("Escape");
  await page.keyboard.press("f");
  await expect(tile(page, LARGE.build)).toBeVisible();
  await tile(page, LARGE.build).dblclick();
  await expect(tile(page, LARGE.build)).toHaveCount(0);
  await expect(nodeCard(page, LARGE.build)).toBeVisible();
});

test("a container opened early, below the reading zoom, draws its state chip inside its head", async ({ page }) => {
  const id = await fitted(page);
  await open(page, "browser", `/journeys/${id}/nodes/${LARGE.helpers}?view=graph&depth=actions`);
  await zoomTo(page, READING);
  // Zoom out until the build stage has opened with its groups drawn as tiles.
  await expect
    .poll(async () => {
      if ((await tile(page, LARGE.partA).count()) === 0) {
        await pinch(page, -1);
      }
      return tile(page, LARGE.partA).count();
    })
    .toBe(1);
  expect((await viewportOf(page)).zoom).toBeLessThan(READING);
  const stage = nodeCard(page, LARGE.build);
  await expect(stage).not.toHaveAttribute("data-tile", "");
  const chip = stage.getByTestId("card-head-chip");
  await expect(chip).toBeVisible();
  const [chipBox, stageBox] = await whenSettled(page, () => Promise.all([chip.boundingBox(), stage.boundingBox()]));
  expect(chipBox?.y).toBeGreaterThanOrEqual(stageBox?.y ?? Infinity);
  expect((chipBox?.x ?? Infinity) + (chipBox?.width ?? 0)).toBeLessThanOrEqual((stageBox?.x ?? 0) + (stageBox?.width ?? 0));
});

test("focus on a container's own title stays on it when the container becomes a tile", async ({ page }) => {
  const id = await fitted(page);
  await open(page, "browser", `/journeys/${id}/nodes/${LARGE.partA}?view=graph&depth=actions`);
  await zoomTo(page, READING);
  await nodeCard(page, LARGE.build).getByTestId("card-open").focus();
  await pinch(page, -8);
  await expect.poll(async () => (await viewportOf(page)).zoom).toBeLessThan(0.2);
  await expect(tile(page, LARGE.build)).toBeVisible();
  await expect.poll(() => page.evaluate(() => (document.activeElement as HTMLElement | null)?.closest(".react-flow__node")?.getAttribute("data-id") ?? "")).toBe(LARGE.build);
});

test("Enter on a tile frames it drawn open", async ({ page }) => {
  await fitted(page);
  await page.locator(`.react-flow__node[data-id="${LARGE.build}"]`).focus();
  await page.keyboard.press("Enter");
  await expect(tile(page, LARGE.build)).toHaveCount(0);
  await expect(nodeCard(page, LARGE.build)).toBeVisible();
});
