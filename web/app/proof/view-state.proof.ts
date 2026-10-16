// The proof's media for one view state across a journey's views
// (briefs/proof/2026-10-16-one-view-state/prove.sh): on a fresh vendor evaluation the viewer is
// made its owner, and two decisions are dated, their titles holding one word: X on the frontier,
// Y waiting on the plan. With Detail at Decisions, Frontier on, Only mine and a search for that word, the
// graph, the list, the timeline and the walk through each draw the same two (the walk through
// only X, its every card on the frontier). Also the timeline's Filter, the graph's View menu on a
// phone, the empty graph with Reset the view, and a short video of the settings following the
// viewer from view to view, written to CAIRN_PROOF_OUT. Steps only wait for the state they
// picture; the e2e specs assert it. Everything runs on the in-browser host, at the fixed day the
// browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/view-state.proof.ts)
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { daysAfter } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { goTo, nodeCard, openFilter, openJourney } from "../e2e/shell.ts";
import { closePopover, retitleAndPin, search, setViewState, viewerOwnsTheJourney } from "../e2e/view-state.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page, ms = 900) => page.waitForTimeout(ms);
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

/** The word the search finds, in X's and Y's titles only. */
const WORD = "shortlist";
const X = "n_purpose";
const Y = "n_comparison_set";

test.use({ viewport: { width: 1280, height: 800 } });

/** A fresh journey with X and Y made the viewer's and dated, open on its graph at every detail. */
async function journeyWithXAndY(page: Page): Promise<void> {
  await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
  const id = await startJourney(page, "browser", journeyName("Evaluation"), { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", id);
  await viewerOwnsTheJourney(page);
  await retitleAndPin(page, X, `Purchase or research for the ${WORD}`, daysAfter(FIXED_TODAY, 3));
  await retitleAndPin(page, Y, `Comparison set for the ${WORD}`, daysAfter(FIXED_TODAY, 20));
}

/** Detail at Decisions, Frontier on, Only mine, and the search for the word. */
const narrow = (page: Page) => setViewState(page, "Decisions", { mine: true }, WORD);

test("one view state on every view", async ({ page }) => {
  await journeyWithXAndY(page);
  await narrow(page);
  await expect(nodeCard(page, Y)).toBeVisible();
  await beat(page);
  await shot(page, "1-graph");

  await goTo(page, "list");
  await page.locator(`[data-testid="list-row"][data-node="${Y}"]`).waitFor();
  await beat(page);
  await shot(page, "2-list");

  await goTo(page, "timeline");
  await page.locator(`[data-testid="timeline-track"][data-node="${Y}"]`).waitFor();
  await beat(page);
  await shot(page, "3-timeline");

  await openFilter(page);
  await beat(page, 400);
  await shot(page, "5-timeline-filter");
  await closePopover(page);

  await page.getByTestId("walk-through").click();
  await page.locator(`[data-testid="triage-card"][data-node="${X}"]`).waitFor();
  await beat(page);
  await shot(page, "4-walk-through");

  await goTo(page, "graph");
  await search(page, "nothing is called this");
  await page.getByTestId("nothing-matches").waitFor();
  await beat(page);
  await shot(page, "7-nothing-matches");

  await search(page, WORD);
  await expect(nodeCard(page, Y)).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Open map" }).click();
  await page.getByTestId("view-button").click();
  await page.getByTestId("view-panel-phone").waitFor();
  await beat(page);
  await shot(page, "6-phone-view-menu");
});

test("the settings follow the viewer, on video", async ({ page }) => {
  await journeyWithXAndY(page);
  await beat(page, 400);
  await page.screencast.start({ path: join(out, "main-flow.webm") });
  await beat(page);
  await narrow(page);
  await expect(nodeCard(page, Y)).toBeVisible();
  await beat(page, 1500);
  for (const view of ["list", "timeline", "summary"]) {
    await goTo(page, view);
    await beat(page, 1600);
  }
  await page.getByTestId("walk-through").click();
  await page.locator(`[data-testid="triage-card"][data-node="${X}"]`).waitFor();
  await beat(page, 1600);
  await goTo(page, "graph");
  await expect(nodeCard(page, Y)).toBeVisible();
  await beat(page, 1500);
  await page.screencast.stop();
});
