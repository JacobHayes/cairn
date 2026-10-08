// What the browser tests of the screens around a journey share (brief 5.5): starting a
// journey from the form, the overview's status buttons, route detail's versions and actions,
// a file exported from it as the browser saves it, and the entities screen's merge.
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { fresh, open, type HostKind } from "./shell.ts";

/** A fixture's route file as it is kept on disk. */
export function fixtureRoute(fixture: string): string {
  return fileURLToPath(new URL(`../../../fixtures/${fixture}/route.yaml`, import.meta.url));
}

/** Starts a journey from the new-journey form at `path`; its id, once it opens on its walkthrough. */
export async function startJourney(page: Page, host: HostKind, name: string, from?: { route: string; version?: number }): Promise<string> {
  const path = from === undefined ? "/new" : `/new?route=${from.route}${from.version === undefined ? "" : `&version=${String(from.version)}`}`;
  await open(page, host, path);
  const form = page.getByTestId("new-journey-form");
  await form.getByLabel("Name").fill(name);
  if (from === undefined) {
    await form.getByLabel("Start from").selectOption("");
  }
  await form.getByRole("button", { name: "Start the journey" }).click();
  await expect(page).toHaveURL(/\/journeys\/j_[a-z0-9_-]+\/triage\?mode=decisions$/);
  return /\/journeys\/(j_[a-z0-9_-]+)\//.exec(page.url())?.[1] ?? "";
}

/** A fresh name for a journey this run starts. */
export const journeyName = (stem: string) => fresh(stem);

/** Journey `id`'s overview, once derived. */
export async function openOverview(page: Page, host: HostKind, id: string): Promise<Locator> {
  await open(page, host, `/journeys/${id}/overview`);
  const overview = page.getByTestId("overview");
  await expect(overview).toBeVisible();
  return overview;
}

/** Moves the overview's journey to `to` (B11), waiting for the status to show it. */
export async function setStatus(page: Page, to: "active" | "completed" | "archived"): Promise<void> {
  await page.getByTestId(`status-${to}`).click();
  await expect(page.getByTestId("overview-status")).toHaveAttribute("data-status", to);
}

/** Route `route`'s detail, once read. */
export async function openRouteDetail(page: Page, host: HostKind, route: string): Promise<Locator> {
  await open(page, host, `/routes/${route}/versions`);
  const detail = page.getByTestId("route-detail");
  await expect(detail).toBeVisible();
  return detail;
}

/** The route's revision as its detail shows it. */
export async function routeRevision(page: Page): Promise<number> {
  return Number(await page.getByTestId("route-revision").getAttribute("data-revision"));
}

/** Presses a route action on its detail and waits for the route to move past the revision shown. */
export async function routeAction(page: Page, name: string | RegExp): Promise<void> {
  const before = await routeRevision(page);
  await page.getByTestId("route-actions").getByRole("button", { name }).click();
  await expect.poll(() => routeRevision(page)).toBeGreaterThan(before);
}

/** The journeys route detail lists on `version`, each with whether an upgrade is available. */
export async function versionJourneys(page: Page, version: number): Promise<Record<string, string>> {
  const rows = page.locator(`[data-testid="version"][data-version="${String(version)}"] [data-testid="version-journey"]`);
  const pairs: [string, string][] = await rows.evaluateAll((items) => items.map((item): [string, string] => [item.getAttribute("data-journey") ?? "", item.getAttribute("data-upgrade") ?? ""]));
  return Object.fromEntries(pairs);
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
