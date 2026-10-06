// The proof's pictures for brief 5.1 (briefs/proof/5.1/prove.sh): a screenshot of each
// acceptance state of the node detail panel and a short video of its main flow, written to
// CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show, so a picture of the
// wrong state fails the run. Everything runs on the in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Browser, type Locator, type Page } from "@playwright/test";

import { annotate, dateChain, flag, openNode, pin, section, state } from "../e2e/detail.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
/**
 * The panel as a person sees it, `focus` scrolled to the top of its view; with no focus, the
 * whole page (list and panel).
 */
async function shot(page: Page, name: string, focus?: Locator): Promise<void> {
  const path = join(out, `${name}.png`);
  if (focus === undefined) {
    await page.screenshot({ path });
    return;
  }
  await focus.evaluate((element) => {
    element.scrollIntoView({ block: "start" });
  });
  await page.getByTestId("node-detail").screenshot({ path });
}
const beat = (page: Page) => page.waitForTimeout(700);

test.use({ viewport: { width: 1280, height: 900 } });

test("explanations: why relevant, why blocked, the date chain, gravity, participation", async ({ page }) => {
  const results = await openNode(page, "browser", "j_vendor_eval", "n_partner_results");
  const relevance = await section(results, "relevance");
  await expect(relevance.getByTestId("relevance-why")).toHaveAttribute("data-status", "not_relevant");
  await shot(page, "1-why-not-relevant", relevance);
  await page.goBack();
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  const due = await dateChain(panel, "Due");
  await dateChain(panel, "Latest start");
  await shot(page, "2-due-chain", due);
  const blocking = await section(panel, "blocking");
  await expect(flag(panel, "blocked")).toBeVisible();
  await shot(page, "3-why-blocked", blocking);
  const participations = await section(panel, "participations");
  await expect(participations.getByTestId("participation")).not.toHaveCount(0);
  await shot(page, "5-participations", participations);
  const comparison = await openNode(page, "browser", "j_bakeoff", "n_comparison");
  const priority = await section(comparison, "priority");
  await expect(priority.getByTestId("leverage-other").locator("li")).not.toHaveCount(0);
  await shot(page, "4-gravity-contributors", priority);
});

test("a later pin rejected, and resolved with a listed move", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await pin(panel, "2026-11-25");
  const conflict = panel.getByTestId("date-conflict");
  await expect(conflict).toBeVisible();
  await shot(page, "6a-pin-rejected-with-chain", conflict);
  await conflict.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(panel.getByTestId("pin-date")).toHaveText("2026-11-20");
  await shot(page, "6b-resolved-by-a-move", panel.getByTestId("pin"));
});

test("a note, a link, an artifact designated and completed, then removed and stale", async ({ page }) => {
  const offer = await openNode(page, "browser", "j_hiring", "n_offer");
  await annotate(offer, "note", "Confirm the **start date** with the candidate before sending.");
  await shot(page, "7-note-added", offer.getByTestId("annotations"));
  const docs = await openNode(page, "browser", "j_launch", "n_docs");
  const link = await annotate(docs, "reference", "https://example.org/docs/reporting");
  await shot(page, "8-link-added", docs.getByTestId("annotations"));
  await link.getByRole("button", { name: "Make it the artifact" }).click();
  await docs.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  await expect(state(docs)).toHaveAttribute("data-status", "done");
  await shot(page, "9-artifact-designated-and-completed", docs.getByTestId("annotations"));
  await docs.locator('[data-testid="annotation"][data-type="artifact"]').getByRole("button", { name: "Remove" }).click();
  await expect(flag(docs, "stale")).toBeVisible();
  await shot(page, "10-artifact-removed-stale", docs.getByTestId("blocking"));
});

test("a message draft rendered with the journey's context", async ({ page }) => {
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_access");
  await expect(panel.getByTestId("draft-text")).toBeVisible();
  await shot(page, "11-message-draft", panel.getByTestId("resources"));
});

test("dark theme and a narrow screen", async ({ browser, baseURL }) => {
  const dark = await browser.newPage({ baseURL: baseURL ?? "", colorScheme: "dark", viewport: { width: 1280, height: 900 } });
  const panel = await openNode(dark, "browser", "j_vendor_eval", "n_final_report");
  await dateChain(panel, "Due");
  await shot(dark, "12-dark-theme");
  await dark.close();
  const narrow = await browser.newPage({ baseURL: baseURL ?? "", viewport: { width: 390, height: 844 } });
  await openNode(narrow, "browser", "j_vendor_eval", "n_final_report");
  expect(await narrow.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(narrow, "13-narrow-screen");
  await narrow.close();
});

/** The main flow: a journey, the final report's due chain, a later pin resolved, an artifact. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 960, height: 600 },
    recordVideo: { dir: join(out, "video"), size: { width: 960, height: 600 } },
  });
  const page = await context.newPage();
  const panel = await openNode(page, "browser", "j_vendor_eval", "n_final_report");
  await beat(page);
  await dateChain(panel, "Due");
  await beat(page);
  await pin(panel, "2026-11-25");
  await expect(panel.getByTestId("date-conflict")).toBeVisible();
  await panel.getByTestId("date-conflict").scrollIntoViewIfNeeded();
  await beat(page);
  await beat(page);
  await panel.locator('[data-testid="resolution"][data-op="shift_pin"]').getByRole("button").click();
  await expect(panel.getByTestId("pin-date")).toHaveText("2026-11-20");
  await beat(page);
  const docs = await openNode(page, "browser", "j_launch", "n_docs");
  const link = await annotate(docs, "reference", "https://example.org/docs/reporting");
  await beat(page);
  await link.getByRole("button", { name: "Make it the artifact" }).click();
  await docs.getByTestId("actions").getByRole("button", { name: "Complete" }).click();
  await expect(state(docs)).toHaveAttribute("data-status", "done");
  await beat(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
