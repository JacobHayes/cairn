// The proof's media for open proposals (briefs/proof/2026-10-17-open-proposals/prove.sh): a
// journey's Proposals tab with a drafted upgrade, the same tab empty, Home's panel, the Routes
// index's Proposed panel, the review of a proposal that creates a route with the rail on Routes,
// the phone's tab and review, and a short video of finding the proposal from the journey and
// from Home. The strip along the bottom is the tab's address (the page has no browser chrome).
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { openJourneyCard } from "../e2e/around.ts";
import { publishVersionTwo, reviewOpen } from "../e2e/proposals.ts";
import { open } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = async (page: Page, name: string) => {
  // The address strip catches up with the tab every 100ms.
  await page.waitForTimeout(300);
  await page.screenshot({ path: join(out, `${name}.png`) });
};
const beat = (page: Page) => page.waitForTimeout(1000);

/** Draws the tab's address in a strip along the bottom of the page, kept current as the app navigates. */
async function addressStrip(page: Page): Promise<void> {
  await page.addInitScript(() => {
    setInterval(() => {
      const body = document.querySelector("body");
      if (body === null) {
        return;
      }
      let strip = document.getElementById("proof-address");
      if (strip === null) {
        strip = document.createElement("div");
        strip.id = "proof-address";
        strip.style.cssText = "position:fixed;left:0;right:0;bottom:0;z-index:99999;padding:6px 14px;font:14px ui-monospace,monospace;background:#16181d;color:#f4f4f2;pointer-events:none;overflow:hidden;white-space:nowrap;text-overflow:ellipsis";
        body.append(strip);
      }
      strip.textContent = `${location.pathname}${location.search}`;
      // On a phone the page scrolls to the bottom, so the strip rides at the top there.
      strip.style.bottom = innerWidth < 720 ? "auto" : "0";
      strip.style.top = innerWidth < 720 ? "0" : "auto";
    }, 100);
  });
}

/** Drafts the scenario journey's upgrade to version 2, and a proposal to save it as a new route. */
async function draftBoth(page: Page): Promise<void> {
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-upgrade").click();
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  await open(page, "browser", "/journeys/j_vendor_eval");
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-save").click();
  const flow = page.getByTestId("save-as-route-flow");
  await flow.getByLabel("Route id").fill("vendor-checklist");
  await flow.getByLabel("Route name").fill("Vendor checklist");
  await flow.getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
}

test("the states, wide and on a phone", async ({ page }) => {
  await addressStrip(page);
  await draftBoth(page);
  await shot(page, "5-review-of-a-new-route");
  await open(page, "browser", "/journeys/j_vendor_eval");
  await page.getByTestId("tab-proposals").click();
  await expect(page.getByTestId("proposals-panel").getByTestId("proposal-row")).toHaveCount(1);
  await shot(page, "1-journey-proposals-tab");
  await open(page, "browser", "/journeys/j_launch/proposals");
  await expect(page.getByTestId("proposals-none")).toBeVisible();
  await shot(page, "2-no-open-proposals");
  await open(page, "browser", "/");
  await expect(page.getByTestId("home-proposals").getByTestId("proposal-row")).toHaveCount(2);
  await shot(page, "3-home-proposals-to-review");
  await open(page, "browser", "/routes");
  await expect(page.getByTestId("proposed-panel").getByTestId("proposal-row")).toHaveCount(1);
  await shot(page, "4-routes-proposed");
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "browser", "/journeys/j_vendor_eval/proposals");
  await expect(page.getByTestId("proposals-panel").getByTestId("proposal-row")).toHaveCount(1);
  await page.waitForTimeout(800);
  await shot(page, "6-phone-journey-proposals");
  await page.getByTestId("proposals-panel").getByRole("link").first().click();
  await reviewOpen(page);
  await shot(page, "7-phone-review");
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1100, height: 720 }, recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 720 } } });
  const page = await context.newPage();
  await addressStrip(page);
  await draftBoth(page);
  await open(page, "browser", "/journeys/j_vendor_eval");
  await beat(page);
  await page.getByTestId("tab-proposals").click();
  await beat(page);
  await page.getByTestId("proposals-panel").getByRole("link").first().click();
  await reviewOpen(page);
  await beat(page);
  await page.getByTestId("review-trail").getByRole("link", { name: "Open proposals" }).click();
  await beat(page);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Home" }).click();
  await expect(page.getByTestId("home-proposals")).toBeVisible();
  await beat(page);
  await page.getByTestId("home-proposals").getByRole("link").first().click();
  await reviewOpen(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
