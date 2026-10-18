// What the browser tests of the screens around a journey share (brief 5.5): starting a
// journey from the form, the journey's status under its `⋯`, route detail's versions and actions,
// a file exported from it as the browser saves it, and the entities screen's merge.
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { expectAddress, fresh, open, type HostKind } from "./shell.ts";

/** A fixture's route file as it is kept on disk. */
export function fixtureRoute(fixture: string): string {
  return fileURLToPath(new URL(`../../../fixtures/${fixture}/route.yaml`, import.meta.url));
}

/** Starts a journey from the new-journey form at `path`; its id, once it opens on its walkthrough. */
export async function startJourney(page: Page, host: HostKind, name: string, from?: { route: string; version?: number }): Promise<string> {
  const path = from === undefined ? "/journeys/new" : `/journeys/new?route=${from.route}${from.version === undefined ? "" : `&version=${String(from.version)}`}`;
  await open(page, host, path);
  const form = page.getByTestId("new-journey-form");
  await form.getByLabel("Name").fill(name);
  if (from === undefined) {
    await form.getByLabel("Start from").selectOption("");
  }
  await form.getByRole("button", { name: "Start the journey" }).click();
  await expect(page).toHaveURL(/\/journeys\/j_[a-z0-9_-]+\?/);
  const id = /\/journeys\/(j_[a-z0-9_-]+)/.exec(page.url())?.[1] ?? "";
  await expectAddress(page, `/journeys/${id}?view=walk&depth=decisions`);
  return id;
}

/** A fresh name for a journey this run starts. */
export const journeyName = (stem: string) => fresh(stem);

/** Journey `id`'s graph, with its journey card, once derived. */
export async function openJourneyCard(page: Page, host: HostKind, id: string): Promise<Locator> {
  await open(page, host, `/journeys/${id}?view=graph`);
  const card = page.getByTestId("journey-card");
  await expect(card).toBeVisible();
  return card;
}

/** Moves the journey to `to` (B11) from the header's `⋯`, Status, waiting for the status to show it: the chip is there while it is not active. */
export async function setStatus(page: Page, to: "active" | "completed" | "archived"): Promise<void> {
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-status").click();
  await page.getByTestId(`status-${to}`).click();
  const chip = page.getByTestId("journey-status");
  if (to === "active") {
    await expect(chip).toHaveCount(0);
  } else {
    await expect(chip).toHaveAttribute("data-status", to);
  }
}

/** Route `route`'s page, once read: its header is up. */
export async function openRoute(page: Page, host: HostKind, route: string, search = ""): Promise<void> {
  await open(page, host, `/routes/${route}${search}`);
  await expect(page.getByTestId("route-header")).toBeVisible();
}

/** The version selector in a route's header: it says which version the views show. */
export function versionSelect(page: Page): Locator {
  return page.getByTestId("version-select");
}

/** Chooses `item` (a test id) in the route header's `⋯`. */
export async function routeMenuItem(page: Page, item: string): Promise<void> {
  await openRouteMenu(page);
  await page.getByTestId(item).click();
}

/** Opens the header's `⋯` unless it is open. */
export async function openRouteMenu(page: Page): Promise<void> {
  if ((await page.getByTestId("route-menu-menu").count()) === 0) {
    await page.getByTestId("route-menu").click();
  }
}

/** Imports `file` as a new draft of its route from the header's `⋯`; the menu stays open until the import has landed. */
export async function importIntoDraft(page: Page, file: string | { name: string; mimeType: string; buffer: Buffer }): Promise<void> {
  await openRouteMenu(page);
  await page.getByTestId("route-menu-menu").getByLabel("Import a file into the draft").setInputFiles(file);
}

/** Publishes the open draft from the header and waits for the page to show the version it made. */
export async function publishDraft(page: Page): Promise<void> {
  await page.getByTestId("publish-draft").click();
  await expect(versionSelect(page)).toContainText(/^Version \d+/);
}

/** Picks `item` (`draft` or a version number) in the version selector. */
export async function pickVersion(page: Page, item: "draft" | number): Promise<void> {
  await versionSelect(page).click();
  await page.getByTestId(`version-${String(item)}`).click();
}

/** The text of the file a click saves. */
export async function savedText(page: Page, click: () => Promise<void>): Promise<string> {
  const [download] = await Promise.all([page.waitForEvent("download"), click()]);
  const path = await download.path();
  return readFile(path, "utf8");
}

/** Merges `merged` into `survivor` on the entities screen and waits for the alias to show. */
export async function mergeEntities(page: Page, survivor: string, merged: string): Promise<void> {
  const form = page.getByTestId("merge-entities");
  await form.getByLabel("Keep").selectOption(survivor);
  await form.getByLabel("Merge into it").selectOption(merged);
  await form.getByRole("button", { name: "Merge" }).click();
  await expect(page.locator(`[data-testid="entity"][data-entity="${survivor}"]`).getByTestId("aliases")).toContainText(merged);
}

/** Adds an entity on the entities screen; its key, once listed. */
export async function addEntity(page: Page, name: string, emails = ""): Promise<string> {
  const form = page.getByTestId("create-entity");
  await form.getByLabel("New entity's name").fill(name);
  await form.getByLabel("New entity's emails").fill(emails);
  await form.getByRole("button", { name: "Add an entity" }).click();
  const row = page.getByTestId("entity").filter({ has: page.getByTestId("entity-name").getByText(name, { exact: true }) });
  await expect(row).toHaveCount(1);
  return (await row.getAttribute("data-entity")) ?? "";
}
