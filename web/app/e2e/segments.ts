// What segments' browser test uses: the security-review segment imported
// from its file and published, and a container's Insert segment here walked to proposal review,
// each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { publishDraft, versionSelect } from "./around.ts";
import { importRoute, menuItem, openFromCanvas, openJourney } from "./shell.ts";

/** The security-review segment's file (fixtures/README.md). */
export const SEGMENT_FILE = fileURLToPath(new URL("../../../fixtures/security-review/segment.yaml", import.meta.url));

/** Imports the segment from its file as a new segment and publishes its draft as version 1. */
export async function publishSegment(page: Page): Promise<void> {
  await importRoute(page, SEGMENT_FILE);
  await expect(page.getByTestId("route-kind")).toBeVisible();
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
}

/** The insert stepper, once open. */
export function stepper(page: Page): Locator {
  return page.getByTestId("insert-stepper");
}

/** Starts inserting a segment under `container` from its inspector's menu, and picks `segment` in the list. */
export async function startInsert(page: Page, journey: string, container: string, segment: string): Promise<void> {
  await openJourney(page, journey);
  await menuItem(await openFromCanvas(page, container), "insert-segment");
  await stepper(page).getByTestId("segment-offer").filter({ hasText: segment }).getByRole("button").first().click();
  await expect(stepper(page)).toHaveAttribute("data-step", "2");
}
