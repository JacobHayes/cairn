// What proposal review's browser tests share : the vendor evaluation's
// version 2 published from its file, the journey's local edits that meet it, a proposal's
// review items and controls, and confirming and applying it, each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { importIntoDraft, openRoute, publishDraft, versionSelect } from "./around.ts";
import { formField, openEditing, openForm, saveForm } from "./authoring.ts";
import { nodeCard, openFromCanvas, type HostKind } from "./shell.ts";

/** The vendor evaluation's version 2 file (fixtures/README.md). */
export const VERSION_TWO = fileURLToPath(new URL("../../../fixtures/vendor-evaluation/route-v2.yaml", import.meta.url));

/** Publishes the vendor evaluation's version 2: its file imported as a draft, then published. */
export async function publishVersionTwo(page: Page, host: HostKind): Promise<void> {
  await openRoute(page, host, "vendor-evaluation");
  await importIntoDraft(page, VERSION_TWO);
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
}

/**
 * Edits the scenario journey where version 2 also changed (the access deliverable's title,
 * the baseline's condition) and where it did not (kickoff's title), as a person does in edit
 * mode.
 */
export async function editWhereVersionTwoChanges(page: Page, host: HostKind): Promise<void> {
  await openEditing(page, host, "j_vendor_eval");
  await openFromCanvas(page, "n_access");
  await formField(page, "title").getByRole("textbox").fill("Access to the environment");
  await saveForm(page);
  await openFromCanvas(page, "n_baseline");
  await openForm(page, "relevance");
  await formField(page, "relevant_when").getByLabel("Comparison").selectOption("equals");
  await saveForm(page);
  await openFromCanvas(page, "n_kickoff");
  await formField(page, "title").getByRole("textbox").fill("Kickoff meeting");
  await saveForm(page);
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
  await expect(page.getByTestId("item-editor")).toHaveAttribute("aria-label", await nodeCard(page, node).getByTestId("title").innerText());
}

/** The review item of kind `item` about `node`, in the item editor of the node picked. */
export function reviewItem(page: Page, item: string, node: string): Locator {
  return page.locator(`[data-testid="review-item"][data-item="${item}"][data-node="${node}"]`);
}

/** The inspector's body scrolls to its end under the wheel, and neither the page nor the workspace scrolls with it (each region scrolls once). */
export async function scrollsOnce(page: Page): Promise<void> {
  const body = page.locator('.inspector-body[data-pane="inspector"]');
  const box = await body.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  for (let step = 0; step < 12; step += 1) {
    await page.mouse.wheel(0, 600);
  }
  await expect.poll(() => body.evaluate((element) => element.scrollHeight - element.clientHeight - element.scrollTop)).toBeLessThanOrEqual(1);
  expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
  expect(await page.locator(".ws-body").evaluate((element) => element.scrollTop)).toBe(0);
}

/** Resolves the conflict on `node` with `resolution`. */
export async function resolve(page: Page, node: string, resolution: string): Promise<void> {
  await pickNode(page, node);
  const item = reviewItem(page, "conflict", node);
  await item.locator(`[data-testid="resolution"][data-resolution="${resolution}"]`).check();
  await expect(item.getByTestId("unresolved")).toHaveCount(0);
}

/** What blocks the apply now, as the footer lists it. */
export async function blockers(page: Page): Promise<string[]> {
  const text = (await page.getByTestId("apply-proposal").getAttribute("data-blockers")) ?? "";
  return text.split(" ").filter(Boolean);
}

/** Saves the reviewer's edits and waits for the proposal's new revision. */
export async function saveEdits(page: Page): Promise<void> {
  const revision = Number(await page.getByTestId("proposal-rail").getAttribute("data-revision"));
  await page.getByTestId("save-proposal").click();
  await expect(page.getByTestId("unsaved")).toHaveCount(0);
  await expect.poll(async () => Number(await page.getByTestId("proposal-rail").getAttribute("data-revision"))).toBeGreaterThan(revision);
}

/** Applies in one click, waiting for the page to say so: the destination as the proposal left it, its band saying the proposal was applied. */
export async function apply(page: Page): Promise<void> {
  await expect.poll(() => blockers(page)).toEqual([]);
  await page.getByTestId("apply-proposal").click();
  await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "applied");
}
