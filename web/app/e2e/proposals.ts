// What proposal review's browser tests and proof share (brief 5.7): the vendor evaluation's
// version 2 published from its file, the journey's local edits that meet it, a proposal's
// review items and controls, and confirming and applying it, each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { openRouteDetail, routeAction } from "./around.ts";
import { formField, openEditing, saveForm } from "./authoring.ts";
import { openFromCanvas, type HostKind } from "./shell.ts";

/** The vendor evaluation's version 2 file (fixtures/README.md). */
export const VERSION_TWO = fileURLToPath(new URL("../../../fixtures/vendor-evaluation/route-v2.yaml", import.meta.url));

/** Publishes the vendor evaluation's version 2: its file imported as a draft, then published. */
export async function publishVersionTwo(page: Page, host: HostKind): Promise<void> {
  await openRouteDetail(page, host, "vendor-evaluation");
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles(VERSION_TWO);
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  await routeAction(page, "Publish the draft");
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
  await formField(page, "relevant_when").getByLabel("Comparison").selectOption("equals");
  await saveForm(page);
  await openFromCanvas(page, "n_kickoff");
  await formField(page, "title").getByRole("textbox").fill("Kickoff meeting");
  await saveForm(page);
}

/** The review screen, once its proposal is read and previewed. */
export async function reviewOpen(page: Page): Promise<Locator> {
  const proposal = page.getByTestId("proposal");
  await expect(proposal).toBeVisible();
  await expect(page.getByTestId("proposal-canvas")).toBeVisible();
  return proposal;
}

/** Review item number `index` that is about `node`, of kind `item`. */
export function reviewItem(page: Page, item: string, node: string): Locator {
  return page.locator(`[data-testid="review-item"][data-item="${item}"][data-node="${node}"]`);
}

/** Resolves the conflict on `node` with `resolution`. */
export async function resolve(page: Page, node: string, resolution: string): Promise<void> {
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
  const revision = Number(await page.getByTestId("proposal").getAttribute("data-revision"));
  await page.getByTestId("save-proposal").click();
  await expect(page.getByTestId("unsaved")).toHaveCount(0);
  await expect.poll(async () => Number(await page.getByTestId("proposal").getAttribute("data-revision"))).toBeGreaterThan(revision);
}

/** Confirms the review and applies, waiting for the proposal to be applied. */
export async function confirmAndApply(page: Page): Promise<void> {
  await page.getByTestId("reviewed").check();
  await expect.poll(() => blockers(page)).toEqual([]);
  await page.getByTestId("apply-proposal").click();
  await expect(page.getByTestId("proposal-status")).toHaveAttribute("data-status", "applied");
}
