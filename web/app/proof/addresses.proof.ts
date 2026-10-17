// The proof's media for one address per journey and node
// (briefs/proof/2026-10-17-one-address-per-journey/prove.sh): Home, the index, a journey at its
// default view, a node link that draws a node the depth hides, and a short video of clicking
// around. The video is of the page, which has no browser chrome, so an address strip showing the
// tab's address is drawn over the bottom of every page, in the pictures and the recording only.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { goTo, goWithin, nodeCard, nodePanel, open, visit } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(900);

/** Draws the tab's address in a strip along the bottom of the page, kept current as the app navigates. */
async function addressStrip(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const draw = (body: HTMLElement) => {
      let strip = document.getElementById("proof-address");
      if (strip === null) {
        strip = document.createElement("div");
        strip.id = "proof-address";
        strip.style.cssText = "position:fixed;left:0;right:0;bottom:0;z-index:99999;padding:6px 14px;font:14px ui-monospace,monospace;background:#16181d;color:#f4f4f2;pointer-events:none";
        body.append(strip);
      }
      strip.textContent = `${location.pathname}${location.search}`;
    };
    setInterval(() => {
      const body = document.querySelector("body");
      if (body !== null) {
        draw(body);
      }
    }, 100);
  });
}

test("Home, the index, a journey and a node link", async ({ page }) => {
  await addressStrip(page);
  await visit(page, "browser", "/");
  await expect(page.getByTestId("home-journey").first()).toBeVisible();
  await shot(page, "1-home");
  await visit(page, "browser", "/journeys");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  // The figures are derived in the tab: wait for the first row's progress.
  await expect(page.getByTestId("journey-row").nth(1).locator("td").nth(3)).not.toBeEmpty();
  await shot(page, "2-journeys");
  await visit(page, "browser", "/journeys/j_vendor_eval");
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "summary");
  await expect(page.getByTestId("summary")).toBeVisible();
  await shot(page, "3-journey-summary");
  // A node the depth hides: its card and its stage's are drawn, and the address is the one opened.
  await visit(page, "browser", "/journeys/j_vendor_eval/nodes/n_access?depth=stages");
  await expect(nodeCard(page, "n_access")).toBeVisible();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await page.keyboard.press("f");
  await page.waitForTimeout(1200);
  await shot(page, "4-node-link-draws-the-node");
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1100, height: 720 }, recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 720 } } });
  const page = await context.newPage();
  await addressStrip(page);
  await open(page, "browser", "/");
  await expect(page.getByTestId("home-journey").first()).toBeVisible();
  await beat(page);
  const item = page.getByTestId("home-item").first();
  await item.locator(".node-row-main").first().click();
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  await beat(page);
  await goTo(page, "list");
  await beat(page);
  await goTo(page, "summary");
  await beat(page);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Journeys" }).click();
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await beat(page);
  await goWithin(page, "/journeys/j_vendor_eval/nodes/n_access?depth=stages");
  await expect(nodeCard(page, "n_access")).toBeVisible();
  await beat(page);
  await page.getByRole("navigation", { name: "Screens", exact: true }).getByRole("link", { name: "Routes" }).click();
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
