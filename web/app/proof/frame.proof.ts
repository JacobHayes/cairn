// The proof's media for the app frame (briefs/proof/8.4/prove.sh): a screenshot of the canvas
// with an inspector open, the list, the decision view, the inspector on its own, and the same screens in dark, plus
// the tablet sheet and the phone's map preview. Each step asserts what its picture shows, so a
// picture of the wrong state fails the run. It reads only test ids, so it runs on the frame before
// and after the migration (CAIRN_PROOF_OUT names where the pictures go).
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { openActing } from "../e2e/acting.ts";
import { nodeCard, open, openAt, openFromCanvas, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1440, height: 860 } });

async function canvasWithInspector(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_plan");
  await expect(page.getByTestId("node-detail")).toBeVisible();
}

test("the canvas with the inspector open, light and dark", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await canvasWithInspector(page);
  await shot(page, "canvas-light");
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await expect(page.getByTestId("node-detail")).toBeVisible();
  await shot(page, "canvas-dark");
});

test("the list and its dark twin", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openActing(page, "browser", "j_vendor_eval", "plan/list");
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  await shot(page, "list-light");
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await shot(page, "list-dark");
});

test("the decision graph fills the workspace", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openAt(page, "browser", "j_vendor_eval", "plan/graph?decisions=1");
  await expect(page.getByTestId("canvas")).toBeVisible();
  await shot(page, "decisions-graph");
});

test("the inspector scrolled to the end of its history", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await canvasWithInspector(page);
  const history = page.getByTestId("node-detail").getByTestId("history");
  await history.locator("summary").click();
  await expect(history.getByText("Reading...")).toHaveCount(0);
  await page.mouse.move(1240, 400);
  for (let step = 0; step < 8; step += 1) {
    await page.mouse.wheel(0, 600);
  }
  await expect.poll(() => history.evaluate((element) => element.getBoundingClientRect().bottom)).toBeLessThanOrEqual(860);
  await shot(page, "inspector-history");
});

test("the tablet's bottom sheet", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1024, height: 700 });
  await canvasWithInspector(page);
  await shot(page, "tablet-sheet");
});

test.describe("a phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the page with its map preview, the full-screen map, and a node's sheet", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
    await openJourney(page, "browser", "j_vendor_eval");
    await shot(page, "phone-page");
    await page.getByTestId("map-open").click();
    await expect(page.getByTestId("map-full")).toBeVisible();
    await shot(page, "phone-map");
    await nodeCard(page, "n_plan").getByTestId("card-open").click();
    await expect(page.getByTestId("node-detail")).toBeVisible();
    await shot(page, "phone-sheet");
  });
});

test("the you page offers a theme control", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await open(page, "browser", "/me");
  await page.getByRole("radio", { name: "Dark" }).check();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await shot(page, "you-dark");
});
