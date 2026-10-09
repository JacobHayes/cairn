// The proof's media for the journey pages (briefs/proof/8.6/prove.sh): a screenshot of the
// header, the toolbar, and the inspector on each page and projection, the journey card, the
// lifecycle and journey menus, the filter with its active chips, the Summary page, and a
// phone's menu and a tablet's toolbar and sheet, written to CAIRN_PROOF_OUT. Each step asserts what its
// picture is meant to show, so a picture of the wrong state fails the run. Everything runs on
// the in-browser host, seeded on each load, at the fixed day the browser tests read the
// fixtures on.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { nextKeys, turnOn } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { goTo, nodePanel, openAt } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const fixed = { fixedToday: FIXED_TODAY };

test.use({ viewport: { width: 1280, height: 760 } });

test("the walkthrough of a journey just started, with the card beside it", async ({ page }) => {
  await startJourney(page, "browser", journeyName("Storage vendor evaluation"), { route: "vendor-evaluation", version: 1 });
  await expect(page.getByTestId("chip-decisions")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("triage-card")).toBeVisible();
  await shot(page, "1-walkthrough-on-start");
  await goTo(page, "next", "list");
  await expect(page.getByTestId("projection-list")).toHaveAttribute("aria-current", "page");
  // DECISIONS stays on from one projection to the next.
  await expect.poll(() => nextKeys(page)).toHaveLength(5);
  await expect(page.getByTestId("journey-card")).toBeVisible();
});

test("the next list, the plan's three projections, and the journey card", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "next/list", fixed);
  await expect(page.getByTestId("card-progress")).toBeVisible();
  await shot(page, "2-next-list-and-journey-card");
  await openAt(page, "browser", "j_vendor_eval", "plan/graph", fixed);
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await shot(page, "3-plan-graph");
  await openAt(page, "browser", "j_vendor_eval", "plan/list", fixed);
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  await shot(page, "4-plan-list");
  await openAt(page, "browser", "j_vendor_eval", "plan/timeline", fixed);
  await expect(page.getByTestId("timeline")).toBeVisible();
  await shot(page, "5-plan-timeline");
});

test("the decision view, an answer's effects in the decision's detail, and the filter with its chips", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/graph?decisions=1", { node: "n_partner_runs", ...fixed });
  await expect(page.getByTestId("decision-view")).toBeVisible();
  await expect(nodePanel(page, "n_partner_runs").getByTestId("decision-affects")).toBeVisible();
  await shot(page, "6-decision-view");
  await openAt(page, "browser", "j_launch", "plan/list?kind=milestone", fixed);
  await turnOn(page, "flag-shortfall");
  await shot(page, "7-filter-open");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("active-filter")).toHaveCount(2);
  await expect(page.getByTestId("list-row")).toHaveCount(2);
  await shot(page, "8-active-filter-chips");
});

test("the lifecycle chip and the journey's menu, then the Summary page", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "next/list", fixed);
  await page.getByTestId("journey-menu").click();
  await expect(page.getByTestId("menu-summary")).toBeVisible();
  await shot(page, "9-journey-menu");
  await page.keyboard.press("Escape");
  await page.getByTestId("lifecycle-chip").click();
  await expect(page.getByTestId("status-completed")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByTestId("card-expand").click();
  await expect(page.getByTestId("journey-card")).toHaveAttribute("data-full", "true");
  await shot(page, "10-summary-page");
});

test("a phone's menu, and a tablet's two-row toolbar and bottom sheet", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 780 });
  await openAt(page, "browser", "j_launch", "next/list", fixed);
  await page.getByTestId("journey-menu").click();
  await expect(page.getByTestId("menu-summary")).toBeVisible();
  await shot(page, "11-phone-menu");
  await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 1024, height: 700 });
  await openAt(page, "browser", "j_launch", "plan/timeline", fixed);
  await expect(page.getByTestId("timeline")).toBeVisible();
  await shot(page, "12-tablet-toolbar");
  await openAt(page, "browser", "j_launch", "plan/graph", { node: "n_code_freeze", ...fixed });
  await expect(nodePanel(page, "n_code_freeze")).toBeVisible();
  await shot(page, "13-tablet-sheet");
});
