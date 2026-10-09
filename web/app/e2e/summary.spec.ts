// The Summary page (C18): each fixture's counts and lists as fixtures/README.md states them
// (read on the browser host at the scenario matrix's day), the journey card's way to it, and a
// print of it leaving out the navigation and the detail panel. The server host draws it from
// the page's own derivation of the server's document, which web/wasm's agreement cases hold
// to the server's projection.
import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY, FIXTURE_JOURNEYS } from "./views.ts";

const README = readFileSync(new URL("../../../fixtures/README.md", import.meta.url), "utf8").split("\n");

/** The keys a part of the summary lists, in its order. */
function listed(page: Page, part: string): Promise<string[]> {
  return page
    .locator(`[data-testid="${part}"] [data-testid="summary-item"]`)
    .evaluateAll((items) => items.map((item) => item.getAttribute("data-node") ?? ""));
}

const keys = (list: string[]) => (list.length === 0 ? "none" : list.map((key) => `\`${key}\``).join(", "));

/** The summary on the page as fixtures/README.md states it, one line per fixture. */
async function summaryLine(page: Page, fixture: string): Promise<string> {
  const states = await page
    .getByTestId("summary-state")
    .evaluateAll((rows) => rows.map((row) => `${row.getAttribute("data-state") ?? ""} ${row.getAttribute("data-count") ?? ""}`));
  const remaining = await page.getByTestId("summary-remaining").getAttribute("data-count");
  const upcoming = await page
    .locator('[data-testid="summary-upcoming"] [data-testid="summary-item"]')
    .evaluateAll((items) => items.map((item) => `\`${item.getAttribute("data-node") ?? ""}\` ${item.getAttribute("data-date") ?? ""}`));
  return [
    `- \`${fixture}\`, status summary: ${states.join(", ")}`,
    `remaining ${remaining ?? ""}`,
    `overdue ${keys(await listed(page, "summary-overdue"))}`,
    `short ${keys(await listed(page, "summary-shortfalls"))}`,
    `stale ${keys(await listed(page, "summary-stale"))}`,
    `upcoming ${upcoming.length === 0 ? "none" : upcoming.join(", ")}`,
    `open decisions ${keys(await listed(page, "summary-open"))}.`,
  ].join("; ");
}

for (const [fixture, journey] of Object.entries(FIXTURE_JOURNEYS)) {
  test(`C18: the ${fixture} summary matches the fixture README`, async ({ page }) => {
    await openAt(page, "browser", journey, "summary", { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("summary")).toBeVisible();
    expect(README).toContain(await summaryLine(page, fixture));
  });
}


test("C18: the journey card opens the Summary page, a node opens beside it, and a print leaves out navigation and the panel", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "next/list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("journey-card")).toHaveAttribute("data-full", "false");
  await page.getByTestId("card-expand").click();
  await expect(page).toHaveURL(/\/journeys\/j_bakeoff\/summary$/);
  await expect(page.getByTestId("journey-card")).toHaveAttribute("data-full", "true");
  await page.locator('[data-testid="summary-open"] [data-node="n_winner"] a').click();
  await expect(nodePanel(page, "n_winner")).toBeVisible();
  await expect(page).toHaveURL(/\/journeys\/j_bakeoff\/summary\/nodes\/n_winner$/);
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(page.getByTestId("summary")).toBeVisible();
  // Paper has no dark theme: a dark screen prints its panels in black on white.
  const inked = await page.getByTestId("summary-open").evaluate((part) => {
    const muted = part.querySelector(".muted");
    const figure = document.querySelector("[data-testid=summary-remaining] .summary-figure-count");
    return [part, muted, figure].map((each) => (each === null ? "missing" : `${getComputedStyle(each).backgroundColor} ${getComputedStyle(each).color}`));
  });
  expect(inked).toEqual(["rgb(255, 255, 255) rgb(0, 0, 0)", "rgba(0, 0, 0, 0) rgb(51, 51, 51)", "rgba(0, 0, 0, 0) rgb(0, 0, 0)"]);
  await expect(page.getByTestId("journey-toolbar")).toBeHidden();
  await expect(nodePanel(page, "n_winner")).toBeHidden();
  await expect(page.getByRole("button", { name: "Print" })).toBeHidden();
});
