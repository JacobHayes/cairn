// What the view state's browser tests and proof share: setting Detail, the highlights, the
// Filter and the search from the toolbar the way a person does, and giving a node of a fresh
// journey an owner, a pin and a title the search can find.
import { expect, type Locator, type Page } from "@playwright/test";

import { setFrontier } from "./acting.ts";
import { pin, state } from "./detail.ts";
import { menuItem, nodePanel, openFilter, openFromCanvas, rename, setDetail } from "./shell.ts";

/** The viewer's entity on the in-browser host, as the vendor evaluation names it. */
export const VIEWER = "e_lead";

/** The Filter's popover. */
export const filterOf = (page: Page) => page.getByTestId("filter-panel");

/** Closes the open popover. */
export async function closePopover(page: Page): Promise<void> {
  await page.keyboard.press("Escape");
  await expect(page.locator("[data-popover]")).toHaveCount(0);
}

/** Searches for `text` from the toolbar's search, as a person does. */
export async function search(page: Page, text: string): Promise<void> {
  const box = page.getByTestId("toolbar-search");
  await box.fill(text);
  await box.press("Enter");
}

/** Turns the checkbox inside `check` on or off, once its new state has come back from the address. */
export async function setCheck(check: Locator, on: boolean): Promise<void> {
  const box = check.locator("input");
  if ((await box.isChecked()) !== on) {
    await box.click();
  }
  await (on ? expect(box).toBeChecked() : expect(box).not.toBeChecked());
}

/** On the journey's graph, at every detail: retitles `node` to `title` and pins it to `date`, from its detail as a person does; leaves it closed. */
export async function retitleAndPin(page: Page, node: string, title: string, date: string, assign = false): Promise<void> {
  await rename(page, node, title);
  const panel = await openFromCanvas(page, node);
  if (assign) {
    await menuItem(panel, "assign");
    await panel.getByTestId("assign").getByLabel("Assign owner").selectOption(VIEWER);
    await panel.getByTestId("assign").getByRole("button", { name: "Assign" }).click();
    await expect(panel.getByTestId("participations")).toContainText("Evaluation Lead");
  }
  await pin(panel, date);
  await expect(panel.getByTestId("date").first()).toHaveAttribute("data-origin", /pin/);
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, node)).toHaveCount(0);
}

/** As `retitleAndPin`, and makes the viewer the node's owner from its detail first. */
export const ownAndPin = (page: Page, node: string, title: string, date: string) => retitleAndPin(page, node, title, date, true);

/** On the journey's graph, answers the decision that fills the vendor evaluation's owner role with the viewer, from its detail. */
export async function viewerOwnsTheJourney(page: Page): Promise<void> {
  const panel = await openFromCanvas(page, "n_who_owns");
  const editor = panel.getByTestId("answer-editor");
  await editor.getByLabel("Answer").selectOption(VIEWER);
  await editor.getByRole("button", { name: "Save", exact: true }).click();
  await expect(state(panel)).toHaveAttribute("data-status", "done");
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, "n_who_owns")).toHaveCount(0);
}

/** What the Filter is set to: Only mine, an owner, flags, and the two display-state switches. */
export interface FilterSettings {
  mine?: boolean;
  owner?: string;
  flags?: string[];
  notRelevant?: boolean;
  conditional?: boolean;
}

/** Sets Detail to `detail`, Frontier on, the Filter's `filter`, and the search for `text`, from the toolbar. */
export async function setViewState(page: Page, detail: string, filter: FilterSettings, text: string): Promise<void> {
  await setDetail(page, detail);
  await setFrontier(page, true);
  await openFilter(page);
  const panel = filterOf(page);
  if (filter.mine !== undefined) {
    await setCheck(panel.getByTestId("flag-mine"), filter.mine);
  }
  if (filter.owner !== undefined) {
    await panel.getByTestId("filter-owner").selectOption(filter.owner);
    await expect(panel.getByTestId("filter-owner")).toHaveValue(filter.owner);
  }
  for (const flag of filter.flags ?? []) {
    await setCheck(panel.getByTestId(`flag-${flag}`), true);
  }
  if (filter.notRelevant !== undefined) {
    await setCheck(panel.getByTestId("show-not-relevant"), filter.notRelevant);
  }
  if (filter.conditional !== undefined) {
    await setCheck(panel.getByTestId("show-conditional"), filter.conditional);
  }
  await closePopover(page);
  await search(page, text);
}
