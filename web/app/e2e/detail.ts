// What the node detail's browser tests share: opening a node's detail the way a
// person does (from its card on the journey's canvas), and its sections, forms, and dates.
import type { Locator, Page } from "@playwright/test";

import { openFromCanvas, openJourney, whenSettled } from "./shell.ts";

/** Opens journey `journey` on `host`, then node `node`'s detail from its card on the canvas. */
export async function openNode(page: Page, journey: string, node: string): Promise<Locator> {
  await openJourney(page, journey);
  return openFromCanvas(page, node);
}

/** A section of the panel, unfolded. */
export async function section(panel: Locator, testId: string): Promise<Locator> {
  const found = panel.getByTestId(testId);
  if ((await whenSettled(panel.page(), () => found.getAttribute("open"))) === null) {
    await found.locator("summary").first().click();
  }
  return found;
}

/** Sets node's pin to `date` from the dates section, without waiting for the outcome. */
export async function pin(panel: Locator, date: string): Promise<void> {
  const editor = (await section(panel, "dates")).getByTestId("pin");
  // The form may be open already (a draft kept across a reload).
  if ((await whenSettled(panel.page(), () => editor.getByTestId("pin-form").count())) === 0) {
    await editor.getByRole("button", { name: /^(Pin a date|Change the pin)$/ }).click();
  }
  await editor.getByLabel("Pin date").fill(date);
  await editor.getByTestId("pin-form").getByRole("button", { name: "Save" }).click();
}

/** The panel's state badge. */
export function state(panel: Locator): Locator {
  return panel.getByTestId("detail-state");
}
