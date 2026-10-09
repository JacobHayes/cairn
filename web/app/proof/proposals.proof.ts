// The proof's pictures for brief 5.7 (briefs/proof/5.7/prove.sh): a screenshot of each
// acceptance state of proposal review (an upgrade to the fixture's version 2 with its
// conflicts, resolved and applied; a stale proposal refreshed and reviewed again; the routeless
// journey saved as a route and re-linked; a placeholder broken down from triage) and a short
// video of the upgrade, written to CAIRN_PROOF_OUT. Each step asserts what its picture is
// meant to show, so a picture of the wrong state fails the run. Everything runs on the
// in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { card, openActing } from "../e2e/acting.ts";
import { dismissNotices } from "../e2e/authoring.ts";
import { chooseFromMenu, openJourneyCard, openRouteDetail, routeAction, startJourney } from "../e2e/around.ts";
import { blockers, confirmAndApply, editWhereVersionTwoChanges, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits } from "../e2e/proposals.ts";
import { state } from "../e2e/detail.ts";
import { goWithin, openFromCanvas, openJourney, rename } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
/** A picture of the page, without the saved notices that sit over it. */
async function shot(page: Page, name: string): Promise<void> {
  await dismissNotices(page);
  await page.screenshot({ path: join(out, `${name}.png`), fullPage: true });
}
const beat = (page: Page) => page.waitForTimeout(700);

test.use({ viewport: { width: 1280, height: 900 } });
test.describe.configure({ mode: "serial" });

/** The upgrade to version 2 proposed from the overview, with the journey's edits meeting it; the review open. */
async function proposeUpgrade(page: Page, pictures: boolean): Promise<void> {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await chooseFromMenu(page, "journey-menu", "menu-upgrade");
  await expect(page.getByTestId("upgrade-flow").getByLabel("Upgrade to version")).toHaveValue("2");
  if (pictures) {
    await page.getByTestId("journey-flows").scrollIntoViewIfNeeded();
    await shot(page, "1-overview-proposes");
  }
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
}

test("B7, C14: the upgrade to version 2, its conflicts resolved, applied", async ({ page }) => {
  await proposeUpgrade(page, true);
  await expect(reviewItem(page, "conflict", "n_access").getByTestId("unresolved")).toBeVisible();
  await expect(page.locator('[data-testid="node-card"][data-node="n_signoff"]')).toHaveAttribute("data-trace", "added");
  await shot(page, "2-upgrade-as-a-diff");
  await resolve(page, "n_access", "keep_journey");
  await resolve(page, "n_baseline", "take_route");
  await reviewItem(page, "orphan", "n_workload").getByTestId("orphan-remove").check();
  await expect(page.locator('[data-testid="diff-node"][data-node="n_workload"]')).toHaveAttribute("data-status", "removed");
  await reviewItem(page, "orphan", "n_workload").scrollIntoViewIfNeeded();
  await shot(page, "3-conflicts-resolved");
  await saveEdits(page);
  await confirmAndApply(page);
  await shot(page, "4-applied");
});

test("I6: a stale proposal, refreshed and reviewed again", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await chooseFromMenu(page, "journey-menu", "menu-upgrade");
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  const address = page.url();
  await page.getByTestId("reviewed").check();
  await openJourney(page, "browser", "j_vendor_eval");
  await rename(page, "n_findings", "Findings, drafted");
  await goWithin(page, address);
  await expect(page.getByTestId("stale").getByTestId("intervening-patch")).toHaveCount(1);
  await shot(page, "5-stale");
  await page.getByTestId("refresh").click();
  await expect.poll(() => blockers(page)).toEqual(["unreviewed"]);
  await page.getByTestId("proposal-footer").scrollIntoViewIfNeeded();
  await shot(page, "6-refreshed-review-again");
});

test("B8, B9: the routeless journey saved as a route, and re-linked", async ({ page }) => {
  await openJourneyCard(page, "browser", "j_bakeoff");
  await chooseFromMenu(page, "journey-menu", "menu-save");
  const save = page.getByTestId("save-as-route-flow");
  await save.getByLabel("Route id").fill("bake-off-route");
  await save.getByLabel("Route name").fill("Bake-off");
  await save.getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  const mappings = page.locator('[data-testid="review-item"][data-item="participation"]');
  await mappings.first().getByLabel("New role title").fill("Trial lead");
  await mappings.first().getByRole("button", { name: "Map to a new role" }).click();
  for (const mapping of (await mappings.all()).slice(1)) {
    await mapping.getByLabel("Map to").selectOption("drop");
  }
  await expect(page.getByTestId("unresolved")).toHaveCount(0);
  await mappings.first().scrollIntoViewIfNeeded();
  await shot(page, "7-save-as-route-mapped");
  await saveEdits(page);
  await confirmAndApply(page);
  await openRouteDetail(page, "browser", "bake-off-route");
  await routeAction(page, "Publish the draft");
  await openJourneyCard(page, "browser", "j_bakeoff");
  await chooseFromMenu(page, "journey-menu", "menu-relink");
  await page.getByTestId("relink-flow").getByLabel("Re-link to route").selectOption("bake-off-route");
  await page.getByTestId("relink-flow").getByRole("button", { name: "Propose the re-link" }).click();
  await reviewOpen(page);
  await confirmAndApply(page);
  await openJourneyCard(page, "browser", "j_bakeoff");
  await expect(page.getByTestId("overview-lineage")).toContainText("Bake-off, version 1");
  await shot(page, "8-relinked");
});

test("B10: a placeholder broken down from triage through a proposal", async ({ page }) => {
  const journey = await startJourney(page, "browser", "Broken down", { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", journey);
  const kickoff = await openFromCanvas(page, "n_kickoff");
  await kickoff.getByRole("button", { name: "Mark reached" }).click();
  await expect(state(kickoff)).toHaveAttribute("data-status", "done");
  await openActing(page, "browser", journey, "next/cards?kind=deliverable");
  for (let pass = 0; pass < 4 && (await card(page).getAttribute("data-node")) !== "n_workload"; pass += 1) {
    await card(page).getByTestId("pass").click();
  }
  await card(page).getByTestId("break-down").click();
  const form = card(page).getByTestId("break-down-form");
  await form.getByLabel("Piece title").fill("Ingest workload");
  await form.getByRole("button", { name: "Another piece" }).click();
  await form.getByLabel("Piece title").nth(1).fill("Query workload");
  await shot(page, "9-break-down-from-triage");
  await form.getByTestId("propose-breakdown").click();
  await reviewOpen(page);
  await expect(page.locator('[data-testid="frontier-node"][data-status="new"]')).toHaveCount(2);
  await shot(page, "10-breakdown-with-its-frontier");
  await confirmAndApply(page);
});

test("the dark theme and a narrow screen", async ({ browser, baseURL }) => {
  const dark = await browser.newPage({ baseURL: baseURL ?? "", colorScheme: "dark", viewport: { width: 1280, height: 900 } });
  await proposeUpgrade(dark, false);
  await shot(dark, "11-dark-theme");
  await dark.close();
  const narrow = await browser.newPage({ baseURL: baseURL ?? "", viewport: { width: 390, height: 844 } });
  await proposeUpgrade(narrow, false);
  expect(await narrow.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(narrow, "12-narrow-screen");
  await narrow.close();
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1200, height: 760 }, recordVideo: { dir: join(out, "video"), size: { width: 900, height: 570 } } });
  const page = await context.newPage();
  await proposeUpgrade(page, false);
  await beat(page);
  await resolve(page, "n_access", "keep_journey");
  await beat(page);
  await resolve(page, "n_baseline", "take_route");
  await beat(page);
  await reviewItem(page, "orphan", "n_workload").getByTestId("orphan-remove").check();
  await beat(page);
  await saveEdits(page);
  await confirmAndApply(page);
  await beat(page);
  await page.getByTestId("applied-journey").click();
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
