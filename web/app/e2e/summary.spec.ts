// The status summary (C18): each fixture's counts and lists as fixtures/README.md states them
// (read on the browser host at the scenario matrix's day), the server's projection shown as
// it answers it, the tabs leading to it from the canvas, and a print of it leaving out the
// navigation and the detail panel.
import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { nodePanel, openJourney } from "./shell.ts";
import { FIXTURE_JOURNEYS, openScreen, served } from "./views.ts";

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
    await openScreen(page, "browser", journey, "summary");
    await expect(page.getByTestId("summary")).toBeVisible();
    expect(README).toContain(await summaryLine(page, fixture));
  });
}

test("C18: the summary shows the server's projection", { tag: "@server" }, async ({ page }) => {
  await openScreen(page, "server", "j_launch", "summary");
  const summary = await served<{
    by_state: Record<string, number>;
    remaining: number;
    shortfalls?: string[];
    overdue?: string[];
    upcoming_milestones?: { node: string }[];
    open_decisions?: { node: string }[];
  }>(page, "j_launch", "summary");
  await expect(page.getByTestId("summary-remaining")).toHaveAttribute("data-count", String(summary.remaining));
  for (const [state, count] of Object.entries(summary.by_state)) {
    await expect(page.locator(`[data-testid="summary-state"][data-state="${state}"]`)).toHaveAttribute("data-count", String(count));
  }
  expect(await listed(page, "summary-shortfalls")).toEqual(summary.shortfalls ?? []);
  expect(await listed(page, "summary-overdue")).toEqual(summary.overdue ?? []);
  expect(await listed(page, "summary-upcoming")).toEqual((summary.upcoming_milestones ?? []).map((each) => each.node));
  expect(await listed(page, "summary-open")).toEqual((summary.open_decisions ?? []).map((each) => each.node));
});

test("C18: the tabs lead from the canvas to the summary and keep an open node and the canvas settings, and a print leaves out navigation and the panel", async ({ page }) => {
  await openJourney(page, "browser", "j_bakeoff", "?hide=action&heat=on");
  await page.getByTestId("nav-summary").click();
  await expect(page.getByTestId("screen-summary")).toBeVisible();
  await page.locator('[data-testid="summary-open"] [data-node="n_winner"] a').click();
  await expect(nodePanel(page, "n_winner")).toBeVisible();
  await expect(page).toHaveURL(/#\/journeys\/j_bakeoff\/summary\/nodes\/n_winner\?hide=action&heat=on$/);
  const timelineTab = page.getByTestId("nav-timeline");
  await expect(timelineTab).toHaveAttribute("href", "#/journeys/j_bakeoff/timeline/nodes/n_winner?hide=action&heat=on");
  // The canvas's settings ride along every tab, so going back finds the canvas as it was left.
  const canvasTab = page.getByTestId("nav-canvas");
  await expect(canvasTab).toHaveAttribute("href", "#/journeys/j_bakeoff/nodes/n_winner?hide=action&heat=on");
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(page.getByTestId("summary")).toBeVisible();
  // Paper has no dark theme: a dark screen prints its panels in black on white.
  const inked = await page.getByTestId("summary-open").evaluate((part) => {
    const muted = part.querySelector(".muted");
    const figure = document.querySelector("[data-testid=summary-open-count] .summary-figure-count");
    return [part, muted, figure].map((each) => (each === null ? "missing" : `${getComputedStyle(each).backgroundColor} ${getComputedStyle(each).color}`));
  });
  expect(inked).toEqual(["rgb(255, 255, 255) rgb(0, 0, 0)", "rgba(0, 0, 0, 0) rgb(51, 51, 51)", "rgba(0, 0, 0, 0) rgb(0, 0, 0)"]);
  await expect(page.getByTestId("journey-nav")).toBeHidden();
  await expect(nodePanel(page, "n_winner")).toBeHidden();
  await expect(page.getByRole("button", { name: "Print" })).toBeHidden();
});
