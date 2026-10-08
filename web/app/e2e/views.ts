// What the browser tests of the decision view, the timeline, and the status summary share:
// opening one of a journey's screens on a host, on the browser host at a fixed day so what is
// overdue or due soon does not move with the calendar.
import { expect, type Page } from "@playwright/test";

import { open, type HostKind } from "./shell.ts";

/** The day the browser host's tests read the fixtures on: the scenario matrix's clock. */
export const FIXED_TODAY = "2026-10-06";

/** Each fixture's journey (fixtures/README.md), by the fixture's directory. */
export const FIXTURE_JOURNEYS = {
  "vendor-evaluation": "j_vendor_eval",
  "hiring-loop": "j_hiring",
  "product-launch": "j_launch",
  "bake-off": "j_bakeoff",
} as const;

/**
 * Opens journey `journey`'s screen `segment` (`decisions`, `timeline`, `summary`), with node
 * `node`'s detail beside it when given, once derived. On the browser host the page's clock is
 * held at noon UTC on `FIXED_TODAY` first, so the host derives with that today.
 */
export async function openScreen(page: Page, host: HostKind, journey: string, segment: string, node?: string): Promise<void> {
  if (host === "browser") {
    await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
  }
  await open(page, host, `/journeys/${journey}/${segment}${node === undefined ? "" : `/nodes/${node}`}`);
  await expect(page.getByTestId(`screen-${segment}`)).toBeVisible();
  if (host === "browser") {
    await expect(page.getByTestId("derivation")).toHaveAttribute("data-today", FIXED_TODAY);
  }
}
