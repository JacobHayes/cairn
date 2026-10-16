// The proof's media for the Depth control (briefs/proof/2026-10-16-depth/prove.sh): the Depth
// menu on the graph, the graph at Decisions hidden and faded, the list at Decisions with its
// Answer, Why and Affects columns, the timeline at Deliverables, the walk through at Decisions
// on a journey just started from the vendor-evaluation route, the graph's View menu on a phone,
// and a short video of the graph stepping through the four depths and then Fade on and off,
// written to CAIRN_PROOF_OUT. Steps only wait for the state they picture; the e2e specs assert
// it. Everything runs on the in-browser host, at the fixed day the browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/depth.proof.ts)
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { journeyName, startJourney } from "../e2e/around.ts";
import { goTo, nodeCard, openAt, setDepth, setFade } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page, ms = 900) => page.waitForTimeout(ms);
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const fixed = { fixedToday: FIXED_TODAY };
const JOURNEY = "j_vendor_eval";

test.use({ viewport: { width: 1280, height: 800 }, colorScheme: "light" });

/** Hides the inspector, so the view has the page's width, unless it is already hidden. */
async function hideInspector(page: Page): Promise<void> {
  const hide = page.getByRole("button", { name: "Hide inspector" });
  if (await hide.isVisible()) {
    await hide.click();
  }
  await expect(hide).toHaveCount(0);
}

/** The vendor evaluation's graph at `depth`, once its cards are drawn and fitted. */
async function graphAt(page: Page, query: string): Promise<void> {
  await openAt(page, "browser", JOURNEY, `graph?${query}`, fixed);
  await hideInspector(page);
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await page.waitForFunction(() => document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform !== "translate(0px, 0px) scale(1)");
  await page.keyboard.press("f");
  await beat(page, 500);
}

test("one Depth control on every view", async ({ page }) => {
  await graphAt(page, "depth=decisions");
  await page.getByTestId("depth-button").click();
  await expect(page.getByTestId("depth-panel")).toBeVisible();
  await beat(page);
  await shot(page, "1-depth-menu");
  await page.keyboard.press("Escape");

  await beat(page, 500);
  await shot(page, "2-decisions-hidden");

  await graphAt(page, "depth=decisions&fade=1");
  await beat(page);
  await shot(page, "3-decisions-faded");

  await openAt(page, "browser", JOURNEY, "list?depth=decisions", fixed);
  await hideInspector(page);
  await page.getByTestId("list-row").first().waitFor();
  await beat(page);
  await shot(page, "4-list-decisions");

  await openAt(page, "browser", JOURNEY, "timeline?depth=deliverables", fixed);
  await hideInspector(page);
  await page.getByTestId("timeline-track").first().waitFor();
  await beat(page);
  await shot(page, "5-timeline-deliverables");

  const id = await startJourney(page, "browser", journeyName("Evaluation"), { route: "vendor-evaluation", version: 1 });
  await expect(page.getByTestId("triage-card").first()).toBeVisible();
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Decisions");
  await hideInspector(page);
  await beat(page);
  await shot(page, "6-walk-decisions");

  await openAt(page, "browser", id, "graph?depth=decisions&fade=1");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Open map" }).click();
  await page.getByTestId("view-button").click();
  await page.getByTestId("view-panel-phone").waitFor();
  await beat(page);
  await shot(page, "7-phone-view-menu");
});

test("the graph steps through the depths, on video", async ({ page }) => {
  await graphAt(page, "depth=stages");
  await beat(page, 400);
  await page.screencast.start({ path: join(out, "main-flow.webm") });
  await beat(page, 1200);
  for (const word of ["Decisions", "Deliverables", "Actions", "Decisions"]) {
    await setDepth(page, word);
    await page.keyboard.press("f");
    await beat(page, 1500);
  }
  await setFade(page, true);
  await expect(nodeCard(page, "n_purpose")).toBeVisible();
  await beat(page, 1800);
  await setFade(page, false);
  await beat(page, 1500);
  await goTo(page, "list");
  await beat(page, 1500);
  await page.screencast.stop();
});
