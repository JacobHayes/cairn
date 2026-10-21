// What the browser tests of the screens around a journey share (brief 5.5): starting a
// journey from the form, the journey's status under its `⋯`, route detail's versions and actions,
// a file exported from it as the browser saves it, and the identities screen's merge.
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { expect, type Locator, type Page } from "@playwright/test";

import { expectAddress, fresh, open, openSheet, type HostKind } from "./shell.ts";

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

/** The identity's row in the table. */
export function identityRow(page: Page, key: string): Locator {
  return page.locator(`[data-testid="identity"][data-identity="${key}"]`);
}

/** Selects the identities `keys` with their row checkboxes: one opens alone, two or more are a set. */
export async function selectIdentities(page: Page, ...keys: string[]): Promise<void> {
  for (const key of keys) {
    await identityRow(page, key).getByRole("checkbox").check();
  }
  await expect(page.getByTestId(keys.length > 1 ? "selection-panel" : "identity-detail")).toBeAttached();
  await openSheet(page);
}

/** Merges `merged` into `survivor` from the selection's panel, and waits for the survivor to show the alias. */
export async function mergeIdentities(page: Page, survivor: string, merged: string): Promise<void> {
  await selectIdentities(page, survivor, merged);
  const panel = page.getByTestId("merge-identities");
  await panel.locator(`input[data-identity="${survivor}"]`).check();
  await panel.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(page.getByTestId("identity-detail").getByTestId("aliases")).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/identities/${survivor}$`));
}

/** Adds an identity with Add, and returns its key once it is open in the inspector. */
export async function addIdentity(page: Page, name: string, emails: readonly string[] = []): Promise<string> {
  await page.getByTestId("new-identity").click();
  const form = page.getByTestId("create-identity");
  // The sheet is opened once the form is in it: before that there is no handle to open.
  await expect(form).toBeAttached();
  await openSheet(page);
  await form.getByLabel("Name").fill(name);
  const field = form.getByLabel("Emails");
  for (const email of emails) {
    await field.fill(email);
    await field.press("Enter");
  }
  await form.getByRole("button", { name: "Add identity" }).click();
  await expect(page).toHaveURL(/\/identities\/who_[a-z2-7]{16}$/);
  await expect(page.getByTestId("identity-detail").getByTestId("detail-title")).toHaveText(name);
  const key = new URL(page.url()).pathname.split("/").pop() ?? "";
  // The identity made is the one selected, at its address.
  await expect(identityRow(page, key)).toHaveAttribute("data-selected", "true");
  return key;
}
