// Home and the two indexes in Chromium (C16, C17): Home's rows open a node and act in place, its
// bounded groups, the Journeys table at a phone's width, and the Routes type control. What each
// value is belongs to the models' tests; this is what only a browser shows. In-browser host.
import { expect, test } from "./test.ts";

import { open } from "./shell.ts";

test("C16: a Home row opens its node's address, and Done on a row acts in place", async ({ page }) => {
  await open(page, "browser", "/");
  const group = page.getByTestId("home-journey").first();
  await expect(group).toBeVisible();
  const row = page.getByTestId("home-item").filter({ has: page.getByRole("button", { name: "Done", exact: true }) }).first();
  const node = (await row.getAttribute("data-node")) ?? "";
  await row.hover();
  await row.getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.locator(`[data-testid="home-item"][data-node="${node}"]`)).toHaveCount(0);
  await page.getByTestId("home-item").first().locator("a.node-title").click();
  await expect(page).toHaveURL(/\/journeys\/[^/]+\/nodes\//);
});

test("C16, C17: the Journeys index at 390px shows each row's lead fact and nothing wider than the window", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "browser", "/journeys");
  const rows = page.getByTestId("journey-row");
  await expect(rows.first()).toBeVisible();
  await expect(page.getByTestId("row-reading")).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const widths = await rows.evaluateAll((each) => each.map((one) => one.getBoundingClientRect().right));
  for (const right of widths) {
    expect(right).toBeLessThanOrEqual(390);
  }
  // Each row that leads with a fact shows it in the cell that leads, and only that one.
  const led = page.locator("[data-testid=journey-row][data-lead=trouble], [data-testid=journey-row][data-lead=for-you]");
  expect(await led.count()).toBeGreaterThan(0);
  for (const row of await led.all()) {
    const lead = (await row.getAttribute("data-lead")) === "trouble" ? "trouble" : "for-you";
    const other = lead === "trouble" ? "for-you" : "trouble";
    await expect(row.locator(`td[data-column="${lead}"]`)).toBeVisible();
    await expect(row.locator(`td[data-column="${other}"]`)).toBeHidden();
  }
});

test("C17: the Routes type control writes ?type= and keeps only that type's rows", async ({ page }) => {
  await open(page, "browser", "/routes");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  const all = await page.getByTestId("route-row").count();
  await page.getByTestId("routes-types").getByText("Segments", { exact: true }).click();
  await expect(page).toHaveURL(/[?&]type=segments/);
  await expect(page.getByTestId("route-use")).toHaveCount(0);
  await page.getByTestId("routes-types").getByText("Routes", { exact: true }).click();
  await expect(page).toHaveURL(/[?&]type=routes/);
  await expect(page.getByTestId("segment-use")).toHaveCount(0);
  expect(await page.getByTestId("route-row").count()).toBeLessThanOrEqual(all);
});
