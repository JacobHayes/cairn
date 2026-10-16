// The proof's media for the centered walkthrough (briefs/proof/2026-10-16-centered-walkthrough/
// prove.sh): the walkthrough's card at a desktop width, at a phone width, and the decision
// walkthrough with its introduction at the desktop width, written to CAIRN_PROOF_OUT. Steps only
// wait for the state they picture. Everything runs on the in-browser host, seeded on each load,
// at the fixed day the browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/walkthrough-center.proof.ts)
import { join } from "node:path";

import { test, type Page } from "@playwright/test";

import { startJourney, journeyName } from "../e2e/around.ts";
import { openAt } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page) => page.waitForTimeout(900);
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const fixed = { fixedToday: FIXED_TODAY };

test("the walkthrough on a desktop", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openAt(page, "browser", "j_vendor_eval", "walk", fixed);
  await page.getByTestId("triage-card").waitFor();
  await beat(page);
  await shot(page, "1-desktop");
});

test("the walkthrough on a phone", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openAt(page, "browser", "j_vendor_eval", "walk", fixed);
  await page.getByTestId("triage-card").waitFor();
  await beat(page);
  await shot(page, "2-phone");
});

test("the decision walkthrough on a new journey", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await startJourney(page, "browser", journeyName("Centered"), { route: "vendor-evaluation", version: 1 });
  await page.getByTestId("walkthrough-intro").waitFor();
  await beat(page);
  await shot(page, "3-decisions-desktop");
});
