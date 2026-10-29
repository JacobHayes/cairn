// Home and the two indexes in Chromium (C16, C17): Home's rows open a node and act in place, its
// bounded groups, the Journeys table at a phone's width, and the Routes type control. What each
// value is belongs to the models' tests; this is what only a browser shows. On cairn demo.
import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { nodeCard, nodePanel, open, readable, whenSettled } from "./shell.ts";

test("C16, C17: Done on a Home row acts in place and the address stays `/`, a row opens its node's address, Home ends with a link to every journey, and the rail leads to Routes, whose Type filter writes ?type=", async ({ page }) => {
  await readable(page);
  await open(page, "/");
  const group = page.getByTestId("home-journey").first();
  await expect(group).toBeVisible();
  await expect(page.getByTestId("journey-table")).toHaveCount(0);
  const row = page.getByTestId("home-item").filter({ has: page.getByRole("button", { name: "Mark done", exact: true }) }).first();
  const node = (await whenSettled(page, () => row.getAttribute("data-node"))) ?? "";
  // A row's actions are there without a hover (touch and keyboard users never hover).
  await expect(row.getByRole("button", { name: "Mark done", exact: true })).toBeVisible();
  await row.getByRole("button", { name: "Mark done", exact: true }).click();
  await expect(page.locator(`[data-testid="home-item"][data-node="${node}"]`)).toHaveCount(0);
  expect(new URL(page.url()).pathname + new URL(page.url()).search).toBe("/");
  await checkpoint(page);
  // Its ready rows carry no status chip; a row's title opens its node on the graph.
  await expect(page.getByTestId("home-item").locator(".badge")).toHaveCount(0);
  const next = page.getByTestId("home-item").first();
  const opened = (await whenSettled(page, () => next.getAttribute("data-node"))) ?? "";
  await next.locator("a.node-title").click();
  await expect(page).toHaveURL(/\/journeys\/[^/]+\/nodes\//);
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  await expect(nodePanel(page, opened)).toBeVisible();
  await expect(nodeCard(page, opened)).toBeVisible();
  await checkpoint(page);
  await open(page, "/");
  await page.getByTestId("home-all-journeys").click();
  await expect(page).toHaveURL(/\/journeys$/);
  await checkpoint(page);
  // The Routes Type filter writes ?type= and keeps only that type's rows.
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Routes" }).click();
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  const all = await whenSettled(page, () => page.getByTestId("route-row").count());
  await page.getByLabel("Type").selectOption("segments");
  await expect(page).toHaveURL(/[?&]type=segments/);
  await expect(page.getByTestId("route-use")).toHaveCount(0);
  await page.getByLabel("Type").selectOption("routes");
  await expect(page).toHaveURL(/[?&]type=routes/);
  await expect(page.getByTestId("segment-use")).toHaveCount(0);
  expect(await whenSettled(page, () => page.getByTestId("route-row").count())).toBeLessThanOrEqual(all);
});

test("C16, C17: the Journeys index at 390px shows both of each row's facts and nothing wider than the window", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "/journeys");
  const rows = page.getByTestId("journey-row");
  await expect(rows.first()).toBeVisible();
  await expect(page.getByTestId("row-reading")).toHaveCount(0);
  const widths = await whenSettled(page, () => rows.evaluateAll((each) => each.map((one) => one.getBoundingClientRect().right)));
  for (const right of widths) {
    expect(right).toBeLessThanOrEqual(390);
  }
  // Both independent facts show on a row that has them (what needs a look, what is yours), a fact that says nothing is left out, and an upgrade notice is never hidden.
  let shown = 0;
  const empty = { trouble: ["none"], "for-you": ["nothing", "nothing-open", "inactive"] };
  for (const row of await whenSettled(page, () => rows.all())) {
    for (const [column, attribute] of [["trouble", "data-kind"], ["for-you", "data-for-you"]] as const) {
      const cell = row.locator(`td[data-column="${column}"]`);
      if (empty[column].includes((await whenSettled(page, () => cell.getAttribute(attribute))) ?? "")) {
        await expect(cell, `${column} that says nothing`).toBeHidden();
      } else {
        await expect(cell, `${column} on a row that has one`).toBeVisible();
        shown += 1;
      }
    }
    for (const notice of await whenSettled(page, () => row.getByTestId("upgrade").all())) {
      await expect(notice).toBeVisible();
    }
  }
  expect(shown).toBeGreaterThan(0);
});
