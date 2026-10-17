// The proof's media for Home and the indexes (briefs/proof/2026-10-17-home-and-indexes/prove.sh):
// Home, the Journeys index and the Routes index on the fixtures, on a desktop, in the dark theme
// and on a phone, and a short video of the main flow, written to CAIRN_PROOF_OUT. Steps only wait
// for the state they picture; the e2e specs assert it. Everything runs on the in-browser host.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { open } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page, ms = 900) => page.waitForTimeout(ms);
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1400, height: 900 }, colorScheme: "light" });

test("Home, the Journeys index and the Routes index", async ({ page }) => {
  await open(page, "browser", "/");
  await expect(page.getByTestId("home-item").first()).toBeVisible();
  await beat(page);
  await shot(page, "1-home");
  await page.emulateMedia({ colorScheme: "dark" });
  await beat(page, 400);
  await shot(page, "2-home-dark");
  await page.emulateMedia({ colorScheme: "light" });

  await open(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(page.getByTestId("row-reading")).toHaveCount(0);
  await beat(page);
  await shot(page, "3-journeys");
  await page.emulateMedia({ colorScheme: "dark" });
  await beat(page, 400);
  await shot(page, "4-journeys-dark");
  await page.emulateMedia({ colorScheme: "light" });

  await open(page, "browser", "/routes");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await beat(page);
  await shot(page, "5-routes");

  await page.setViewportSize({ width: 390, height: 1500 });
  await open(page, "browser", "/");
  await expect(page.getByTestId("home-item").first()).toBeVisible();
  await beat(page);
  await shot(page, "6-home-phone");
  await open(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(page.getByTestId("row-reading")).toHaveCount(0);
  await beat(page);
  await shot(page, "7-journeys-phone");
  await open(page, "browser", "/routes");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await beat(page);
  await shot(page, "8-routes-phone");
});

test("the main flow", async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 1200, height: 760 }, recordVideo: { dir: join(out, "video"), size: { width: 1200, height: 760 } } });
  const page = await context.newPage();
  await open(page, "browser", "/");
  await expect(page.getByTestId("home-item").first()).toBeVisible();
  await beat(page, 1500);
  await page.getByTestId("home-item").first().locator("a.node-title").click();
  await beat(page, 1500);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Journeys" }).click();
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await beat(page, 1800);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Routes" }).click();
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await beat(page, 1000);
  await page.getByTestId("routes-types").getByText("Segments", { exact: true }).click();
  await beat(page, 1500);
  const video = page.video();
  await context.close();
  await video?.saveAs(join(out, "main-flow.webm"));
});
