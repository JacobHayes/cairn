// What proposal review's browser tests share: the vendor evaluation's version 2 published from
// its file, a drafted breakdown, a proposal's review items and controls, and confirming and
// applying it, each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { importIntoDraft, journeyName, openRoute, publishDraft, startJourney, versionSelect } from "./around.ts";
import { nodeCard, openAt, settled, whenSettled } from "./shell.ts";

/** The vendor evaluation's version 2 file (fixtures/README.md). */
export const VERSION_TWO = fileURLToPath(new URL("../../../fixtures/vendor-evaluation/route-v2.yaml", import.meta.url));

/** Publishes the vendor evaluation's version 2: its file imported as a draft, then published. */
export async function publishVersionTwo(page: Page): Promise<void> {
  await openRoute(page, "vendor-evaluation");
  await importIntoDraft(page, VERSION_TWO);
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
}

/** Starts a journey from the vendor evaluation and drafts a breakdown of its workload placeholder into one piece, as a person does from its node; the journey's id and the review's address. */
export async function draftBreakdown(page: Page): Promise<{ journey: string; reviewed: string }> {
  const journey = await startJourney(page, journeyName("Breakdown"), { route: "vendor-evaluation" });
  await openAt(page, journey, "list", { node: "n_workload" });
  await page.getByTestId("actions").getByRole("button", { name: /^Break down/ }).click();
  const form = page.getByTestId("break-down-form");
  await form.getByLabel("Piece title").fill("Size the workload");
  await form.getByTestId("propose-breakdown").click();
  await reviewOpen(page);
  const reviewed = new URL(page.url()).pathname;
  expect(reviewed).toMatch(new RegExp(`^/journeys/${journey}/proposals/pr_[0-9a-f]+$`));
  return { journey, reviewed };
}

/** The review rail, once the proposal's layer is on its destination's page and the proposal is read and previewed. */
export async function reviewOpen(page: Page): Promise<Locator> {
  await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "reviewing");
  const rail = page.getByTestId("proposal-rail");
  await expect(rail).toBeAttached();
  await expect(page.getByTestId("canvas")).toBeVisible();
  return rail;
}

/** Picks `node`'s card on the layer, which puts its item editor in the inspector. The layer opens on its first change, so the card may be off screen: the click is sent to the card itself, as panning to it would. */
export async function pickNode(page: Page, node: string): Promise<void> {
  await nodeCard(page, node).getByTestId("card-open").dispatchEvent("click");
  await expect(page.getByTestId("item-editor")).toHaveAttribute("aria-label", await whenSettled(page, () => nodeCard(page, node).getByTestId("title").innerText()));
}

/** What blocks the apply now, as the footer lists it. */
export async function blockers(page: Page): Promise<string[]> {
  const text = (await whenSettled(page, () => page.getByTestId("apply-proposal").getAttribute("data-blockers"))) ?? "";
  return text.split(" ").filter(Boolean);
}

/** Applies in one click, waiting for the page to say so: the destination as the proposal left it, its band saying the proposal was applied, and the page settled. */
export async function apply(page: Page): Promise<void> {
  await expect.poll(() => blockers(page)).toEqual([]);
  await page.getByTestId("apply-proposal").click();
  await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "applied");
  await settled(page);
}

/** The room the graph's view has on the page: the canvas, less what the inspector's sheet covers of its lower part (a column beside it covers none), and the zoom the view is at. */
export async function graphRoom(page: Page): Promise<{ left: number; top: number; right: number; bottom: number; zoom: number }> {
  const column = page.locator(".inspector").locator("xpath=..");
  const [canvas, sheet, zoom, position] = await whenSettled(page, () =>
    Promise.all([
      page.getByTestId("canvas").boundingBox(),
      column.boundingBox(),
      page.locator(".react-flow__viewport").evaluate((element) => new DOMMatrix(getComputedStyle(element).transform).a),
      column.evaluate((element) => getComputedStyle(element).position),
    ]),
  );
  if (canvas === null) {
    throw new Error("the canvas has no box");
  }
  const sheeted = sheet !== null && position === "fixed";
  return { left: canvas.x, top: canvas.y, right: canvas.x + canvas.width, bottom: sheeted ? Math.min(canvas.y + canvas.height, sheet.y) : canvas.y + canvas.height, zoom };
}

/** How far card `node` lies inside `room` on its tightest side (px): negative when it is clipped or under the sheet. */
export async function marginIn(page: Page, node: string, room: { left: number; top: number; right: number; bottom: number }): Promise<number> {
  const box = await whenSettled(page, () => nodeCard(page, node).boundingBox());
  if (box === null) {
    throw new Error(`${node} has no box`);
  }
  return Math.min(box.x - room.left, box.y - room.top, room.right - (box.x + box.width), room.bottom - (box.y + box.height));
}
