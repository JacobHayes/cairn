// What authoring's browser tests and proof share (brief 5.6): starting a route with an empty
// draft, adding a node from the palette and reaching its structure, a node form's fields,
// roles, and a journey's edit mode, each the way a person does it.
import { expect, type Locator, type Page } from "@playwright/test";

import { fresh, open, openJourney, type HostKind } from "./shell.ts";

/** A fresh name for a route this run starts. */
export const routeName = (stem: string) => fresh(stem);

/** Starts a route with an empty draft from the route index; its id, once its draft's canvas opens. */
export async function newRoute(page: Page, host: HostKind, name: string): Promise<string> {
  await open(page, host, "/library");
  const form = page.getByTestId("new-route");
  await form.getByLabel("New route name").fill(name);
  const id = await form.getByLabel("New route id").inputValue();
  await form.getByRole("button", { name: "Start a route with an empty draft" }).click();
  await expect(page.getByTestId("route-graph")).toHaveAttribute("data-status", "draft");
  return id;
}

/** The open node's structure: a draft node's panel, or a journey node's in edit mode. */
export function structure(page: Page): Locator {
  return page.getByTestId("authoring");
}

/** Picks the option of a node picker whose label starts with the node's title. */
export async function pickByTitle(select: Locator, title: string): Promise<void> {
  const value = await select.locator("option").filter({ hasText: new RegExp(`^${title}( \\(|$)`) }).first().getAttribute("value");
  await select.selectOption(value ?? "");
}

/** Adds a node from the palette, at the top level unless `inside` names a container, and waits for its structure to open; its key. */
export async function addNode(page: Page, kind: string, title: string, inside?: string): Promise<string> {
  const palette = page.getByTestId("add-node");
  await palette.getByLabel("Kind").selectOption(kind);
  await palette.getByLabel("New node title").fill(title);
  if (inside === undefined) {
    await palette.getByLabel("Inside").selectOption("");
  } else {
    await pickByTitle(palette.getByLabel("Inside"), inside);
  }
  await palette.getByRole("button", { name: "Add" }).click();
  const panel = structure(page);
  await expect(panel.getByTestId("node-form").getByLabel("Title", { exact: true })).toHaveValue(title);
  return (await panel.getAttribute("data-node")) ?? "";
}

/** The open node's form. */
export function nodeForm(page: Page): Locator {
  return structure(page).getByTestId("node-form");
}

/** One field of the open node's form. */
export function formField(page: Page, field: string): Locator {
  return nodeForm(page).locator(`[data-testid="author-field"][data-field="${field}"]`);
}

/** Saves the open node's form and waits for its changes to land (nothing left to save). */
export async function saveForm(page: Page): Promise<void> {
  const save = page.getByTestId("node-form-save");
  await expect(nodeForm(page).getByTestId("preview")).toHaveAttribute("data-status", "accepted");
  await save.click();
  await expect(save).toHaveText(/^Save\s*$/);
}

/** Adds a role from the roles panel; waits for it to be listed. */
export async function addRole(page: Page, title: string, multi = false): Promise<void> {
  const panel = page.getByTestId("roles-and-kinds");
  if ((await panel.getAttribute("open")) === null) {
    await panel.locator("summary").first().click();
  }
  const roles = panel.getByTestId("roles");
  await roles.getByRole("button", { name: "Add a role" }).click();
  const form = roles.getByTestId("role-form");
  await form.getByLabel("role label").fill(title);
  if (multi) {
    await form.getByRole("checkbox").check();
  }
  await form.getByRole("button", { name: "Add the role" }).click();
  await expect(roles.getByTestId("role").filter({ hasText: title })).toHaveCount(1);
}

/** Opens a structure section of the open node's panel. */
export async function section(page: Page, testId: string): Promise<Locator> {
  const found = structure(page).getByTestId(testId);
  if ((await found.getAttribute("open")) === null) {
    await found.locator("summary").first().click();
  }
  return found;
}

/** A journey's canvas in edit mode on `host`. */
export async function openEditing(page: Page, host: HostKind, journey: string): Promise<void> {
  await openJourney(page, host, journey, "?edit=on");
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
}
