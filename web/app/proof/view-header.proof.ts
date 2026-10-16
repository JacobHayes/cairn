// The proof's media for the journey header (briefs/proof/2026-10-16-view-picker/prove.sh): the
// header line on the graph, the View picker open on the list, the `⋯` menu with Status open, an
// archived journey with its chip and its Status moves, the same archived journey on a phone with
// the picker closed and open, and a short video of the picker taking the page through every view,
// written to CAIRN_PROOF_OUT. Steps only wait for the state they picture; the e2e specs assert
// it. Everything runs on the in-browser host, at the fixed day the browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/view-header.proof.ts)
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { setStatus } from "../e2e/around.ts";
import { goTo, openAt } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page, ms = 900) => page.waitForTimeout(ms);
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const fixed = { fixedToday: FIXED_TODAY };
const JOURNEY = "j_vendor_eval";

test.use({ viewport: { width: 1280, height: 800 }, colorScheme: "light" });

/** The graph with its cards drawn and fitted, over the page's width. */
async function graphDrawn(page: Page): Promise<void> {
  const hide = page.getByRole("button", { name: "Hide inspector" });
  if (await hide.isVisible()) {
    await hide.click();
  }
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.waitForFunction(() => document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform !== "translate(0px, 0px) scale(1)");
  await beat(page, 600);
  await page.keyboard.press("f");
  await beat(page, 800);
}

/** Opens the `⋯` menu with its Status row open. */
async function statusOpen(page: Page): Promise<void> {
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-status").click();
  await expect(page.getByTestId("status-choices")).toBeVisible();
  await beat(page);
}

test("the header, the picker and the status menu", async ({ page }) => {
  await openAt(page, "browser", JOURNEY, "graph", fixed);
  await graphDrawn(page);
  await shot(page, "1-header");

  await goTo(page, "list");
  await page.getByTestId("list-row").first().waitFor();
  await page.getByTestId("view-picker").click();
  await expect(page.getByTestId("view-picker-menu")).toBeVisible();
  await beat(page);
  await shot(page, "2-view-picker");
  await page.keyboard.press("Escape");

  await goTo(page, "graph");
  await graphDrawn(page);
  await statusOpen(page);
  await shot(page, "3-status-menu");
  await page.keyboard.press("Escape");

  await setStatus(page, "archived");
  await statusOpen(page);
  await shot(page, "4-archived");
});

test("an archived journey on a phone", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openAt(page, "browser", JOURNEY, "graph", fixed);
  await setStatus(page, "archived");
  await goTo(page, "list");
  await page.getByTestId("list-row").first().waitFor();
  await beat(page);
  await shot(page, "5-phone-archived");
  await page.getByTestId("view-picker").click();
  await expect(page.getByTestId("view-picker-menu")).toBeVisible();
  await beat(page);
  await shot(page, "6-phone-picker");
});

test("the picker takes the page through every view, on video", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 680 });
  await openAt(page, "browser", JOURNEY, "graph", fixed);
  await graphDrawn(page);
  await page.screencast.start({ path: join(out, "main-flow.webm") });
  await beat(page, 1200);
  for (const view of ["list", "timeline", "summary", "walk", "graph"]) {
    await page.getByTestId("view-picker").click();
    await beat(page, 900);
    await page.getByTestId(`view-${view}`).click();
    await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", view);
    await beat(page, 1500);
  }
  await page.screencast.stop();
});
