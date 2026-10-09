// What the node detail's browser tests and proof share: opening a node's detail the way a
// person does (from its card on the journey's canvas), and its sections, forms, and dates.
import { expect, type Locator, type Page } from "@playwright/test";

import { openFromCanvas, openJourney, type HostKind } from "./shell.ts";

/** Opens journey `journey` on `host`, then node `node`'s detail from its card on the canvas. */
export async function openNode(page: Page, host: HostKind, journey: string, node: string): Promise<Locator> {
  await openJourney(page, host, journey);
  return openFromCanvas(page, node);
}

/** A section of the panel, unfolded. */
export async function section(panel: Locator, testId: string): Promise<Locator> {
  const found = panel.getByTestId(testId);
  if ((await found.getAttribute("open")) === null) {
    await found.locator("summary").first().click();
  }
  return found;
}

/** A date row of the panel (`Due`, `Latest start`, `Earliest start`), its chain unfolded. */
export async function dateChain(panel: Locator, label: string): Promise<Locator> {
  const dates = await section(panel, "dates");
  if (label !== "Due") {
    const more = dates.getByTestId("more-dates");
    if ((await more.getAttribute("open")) === null) {
      await more.locator(":scope > summary").click();
    }
  }
  const row = dates.locator(`[data-testid="date"][data-label="${label}"]`);
  await row.locator("summary").click();
  await expect(row.getByTestId("chain")).toBeVisible();
  return row;
}

/** Sets node's pin to `date` from the dates section, without waiting for the outcome. */
export async function pin(panel: Locator, date: string): Promise<void> {
  const editor = (await section(panel, "dates")).getByTestId("pin");
  await editor.getByRole("button", { name: /^(Pin a date|Change the pin)$/ }).click();
  await editor.getByLabel("Pin date").fill(date);
  await editor.getByTestId("pin-form").getByRole("button", { name: "Save" }).click();
}

/** Adds a note or link from the notes section and waits for it to show. */
export async function annotate(panel: Locator, type: "note" | "artifact" | "reference" | "conversation", text: string): Promise<Locator> {
  const notes = await section(panel, "annotations");
  const before = await notes.getByTestId("annotation").count();
  await notes.getByRole("button", { name: "Add a note or link" }).click();
  await notes.getByLabel("Type").selectOption(type);
  await notes.getByLabel(type === "note" ? "Note" : "Address").fill(text);
  await notes.getByRole("button", { name: "Add", exact: true }).click();
  await expect(notes.getByTestId("annotation")).toHaveCount(before + 1);
  return notes.getByTestId("annotation").last();
}

/** The panel's state badge. */
export function state(panel: Locator): Locator {
  return panel.getByTestId("detail-state");
}

/** The panel's derived flag `flag` (C8: `blocked`, `stale`, ...). */
export function flag(panel: Locator, name: string): Locator {
  return panel.locator(`[data-testid="flag"][data-status="${name}"]`);
}
