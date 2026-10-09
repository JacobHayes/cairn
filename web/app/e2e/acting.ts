// What the acting surfaces' browser tests and proof share: opening the list, the next list,
// and triage the way a person does, reading their rows and cards, and starting a fresh journey
// from the vendor evaluation's route on the server host (the creation screen is 5.5's).
import { readFileSync } from "node:fs";

import { expect, type Locator, type Page } from "@playwright/test";

import { fixtureRoute } from "./around.ts";
import { derivedRevision, fresh, open, openAt, openFilter, type HostKind } from "./shell.ts";

/** Opens journey `journey`'s projection at `path` (`next/list`, `plan/list`, `next/cards`, with a query) on `host`, once derived. */
export async function openActing(page: Page, host: HostKind, journey: string, path: string): Promise<void> {
  await openAt(page, host, journey, path);
}

/** The keys of the rows or items `testId` marks, in the order shown, once there are some. */
export async function keysOf(page: Page, testId: string): Promise<string[]> {
  await expect(page.getByTestId(testId).first()).toBeVisible();
  return page.getByTestId(testId).evaluateAll((rows) => rows.map((row) => row.getAttribute("data-node") ?? ""));
}

/** The next list's items, in order. */
export function nextKeys(page: Page): Promise<string[]> {
  return keysOf(page, "next-item");
}

/** The list's rows, in order. */
export function listKeys(page: Page): Promise<string[]> {
  return keysOf(page, "list-row");
}

/** Node `node`'s item on the next list. */
export function nextItem(page: Page, node: string): Locator {
  return page.locator(`[data-testid="next-item"][data-node="${node}"]`);
}

/** The cards of the current pass, in its order, once the frontier has been read and holds some. */
export async function passOrder(page: Page): Promise<string[]> {
  const triage = page.getByTestId("triage");
  await expect(triage).toHaveAttribute("data-order", /\S/);
  const order = await triage.getAttribute("data-order");
  return (order ?? "").split(" ").filter(Boolean);
}

/** The focus card. */
export function card(page: Page): Locator {
  return page.getByTestId("triage-card");
}

/**
 * Turns the filter checkbox `testId` on, in the toolbar's filter: a filter changes the address,
 * and the box follows it once the screen reads the new address.
 */
export async function turnOn(page: Page, testId: string): Promise<void> {
  await openFilter(page);
  const control = page.getByTestId(testId);
  await control.locator("input").click();
  await expect(control).toHaveAttribute("data-status", "on");
}

/** Selects node `node`'s row on the list, opening the tree if it is folded away. */
export async function select(page: Page, node: string): Promise<void> {
  const row = page.locator(`[data-testid="list-row"][data-node="${node}"]`);
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", /\d/);
  if ((await row.count()) === 0) {
    await page.getByTestId("list-fold-all").click();
  }
  await row.locator("input[type=checkbox]").check();
}

/** Waits until the page's derivation is past `revision`, and returns where it is. */
export async function revisionAfter(page: Page, revision: number): Promise<number> {
  await expect.poll(() => derivedRevision(page)).toBeGreaterThan(revision);
  return derivedRevision(page);
}

/** Answers the focus card's boolean decision with `option` (`yes` or `no`) and, when given, its reason. */
export async function answerCard(page: Page, option: string, why?: string): Promise<void> {
  const editor = card(page);
  await editor.getByRole("radio", { name: new RegExp(`^${option}\\b`, "i") }).check();
  if (why !== undefined) {
    await editor.getByLabel("Why").fill(why);
  }
  await editor.getByRole("button", { name: "Save", exact: true }).click();
}

/** A date `days` after `today`, in the form a date input takes. */
export function daysAfter(today: string, days: number): string {
  const date = new Date(`${today}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

/**
 * Starts a fresh journey from version 1 of the vendor evaluation's route on the server host
 * (B1), as the creation screen will, and returns its id. Each is new, so tests that write to
 * it never meet.
 */
export async function startVendorJourney(page: Page): Promise<string> {
  const id = `j_${fresh("walk").replace(/\W/g, "_").toLowerCase()}`;
  const patchId = `p_${crypto.randomUUID().replaceAll("-", "")}`;
  const response = await page.request.post(`/api/journeys/${id}/patches`, {
    data: {
      patch: {
        id: patchId,
        target: { journey: id },
        base_revision: 0,
        mutations: [{ op: "create_journey", name: "A fresh evaluation", from: { route: "vendor-evaluation", version: 1 } }],
      },
    },
  });
  expect(response.ok(), await response.text()).toBe(true);
  return id;
}

/**
 * Publishes, on the in-browser host, a route like the vendor evaluation with one more decision,
 * relevant only when a partner runs the testing and ranking below the up-front decisions: the
 * partner decision unlocks it along with the partner-led work. Its id.
 */
export async function publishFollowUpRoute(page: Page): Promise<string> {
  const route = "vendor-follow-up";
  const file = readFileSync(fixtureRoute("vendor-evaluation"), "utf8")
    .replace("route: vendor-evaluation\nname: Vendor evaluation\n", `route: ${route}\nname: Vendor evaluation with a follow-up\n`)
    .replace(
      "- key: n_kickoff\n",
      "- key: n_partner_scope\n  id: partner-scope\n  kind: decision\n  title: Partner scope\n  prompt: Does the partner run all of the testing?\n  answer_type: boolean\n  weight: 0\n  relevant_when:\n    equals:\n      decision: partner-runs\n      value: true\n- key: n_kickoff\n",
    );
  await open(page, "browser", "/library");
  await page.getByLabel("Import a route file").setInputFiles({ name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(file) });
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  await page.getByTestId("route-actions").getByRole("button", { name: "Publish the draft" }).click();
  await expect(page.locator('[data-testid="version"][data-version="1"]')).toBeVisible();
  return route;
}
