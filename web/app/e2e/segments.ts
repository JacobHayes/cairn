// What segments' browser test and proof share (brief 7.7): the security-review segment imported
// from its file and published, and a container's Insert segment here walked to proposal review,
// each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { importIntoLibrary, menuItem, open, openFromCanvas, openJourney, type HostKind } from "./shell.ts";

/** The security-review segment's file (fixtures/README.md). */
export const SEGMENT_FILE = fileURLToPath(new URL("../../../fixtures/security-review/segment.yaml", import.meta.url));

/** Imports the segment from its file as a new segment and publishes its draft as version 1. */
export async function publishSegment(page: Page, host: HostKind): Promise<void> {
  await open(page, host, "/library");
  await importIntoLibrary(page, SEGMENT_FILE);
  await expect(page.getByTestId("route-detail")).toHaveAttribute("data-kind", "segment");
  await publishDraft(page);
}

/** From a segment's detail, opens its draft (opening one first when none is) and publishes it. */
export async function publishDraft(page: Page): Promise<void> {
  const edit = page.getByTestId("edit-draft");
  await expect(edit.or(page.getByTestId("open-draft-link"))).toBeVisible();
  if (await edit.isVisible()) {
    await edit.click();
  } else {
    await page.getByTestId("open-draft-link").click();
  }
  await expect(page.getByTestId("route-graph")).toHaveAttribute("data-status", "draft");
  await page.getByTestId("publish-draft").click();
  await expect(page.getByTestId("route-graph")).not.toHaveAttribute("data-status", "draft");
}

/** The insert stepper, once open. */
export function stepper(page: Page): Locator {
  return page.getByTestId("insert-stepper");
}

/** Starts inserting a segment under `container` from its inspector's menu, and picks `segment` in the list. */
export async function startInsert(page: Page, host: HostKind, journey: string, container: string, segment: string): Promise<void> {
  await openJourney(page, host, journey);
  await menuItem(await openFromCanvas(page, container), "insert-segment");
  await stepper(page).getByTestId("segment-offer").filter({ hasText: segment }).getByRole("button").first().click();
  await expect(stepper(page)).toHaveAttribute("data-step", "2");
}
