// What the insertion browser test uses: the security-review route imported
// from its file and published, and a container's Insert route here walked to proposal review,
// each the way a person does it.
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { publishDraft, versionSelect } from "./around.ts";
import { importRoute, menuItem, openFromCanvas, openJourney } from "./shell.ts";

/** The security-review route's file (fixtures/README.md). */
export const ROUTE_FILE = fileURLToPath(new URL("../../../fixtures/security-review/route.yaml", import.meta.url));

/** Imports the route from its file as a new route and publishes its draft as version 1. */
export async function publishRoute(page: Page): Promise<void> {
  await importRoute(page, ROUTE_FILE);
  await expect(versionSelect(page)).toContainText("Draft");
  await publishDraft(page);
}

/** The insert stepper, once open. */
export function stepper(page: Page): Locator {
  return page.getByTestId("insert-stepper");
}

/** Starts inserting a route under `container` from its inspector's menu, and picks `route` in the list. */
export async function startInsert(page: Page, journey: string, container: string, route: string): Promise<void> {
  await openJourney(page, journey);
  await menuItem(await openFromCanvas(page, container), "insert-route");
  await stepper(page).getByTestId("route-offer").filter({ hasText: route }).getByRole("button").first().click();
  await expect(stepper(page)).toHaveAttribute("data-step", "2");
}
