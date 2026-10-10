// The proof's media for the graph-first journey page (briefs/proof/2026-10-10-graph-first/README.md):
// the graph with its Frontier strip and group header rows, the list with the Frontier on, the
// list as a tree with its guide lines, a node's context menu, the walk through, and a zoomed-out
// graph, each in the light and the dark theme, written to CAIRN_PROOF_OUT. Steps only wait for
// the state they picture. Everything runs on the in-browser host, seeded on each load, at the
// fixed day the browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/graph-first.proof.ts)
import { join } from "node:path";

import { test, type Page } from "@playwright/test";

import { nodeCard, openAt } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const beat = (page: Page) => page.waitForTimeout(900);
const fixed = { fixedToday: FIXED_TODAY };

test.use({ viewport: { width: 1440, height: 900 } });

for (const scheme of ["light", "dark"] as const) {
  test.describe(scheme, () => {
    test.use({ colorScheme: scheme });
    const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}-${scheme}.png`) });

    test("the graph, with the Frontier strip and group header rows", async ({ page }) => {
      await openAt(page, "browser", "j_vendor_eval", "graph?detail=work&frontier=1", fixed);
      await page.getByTestId("frontier-strip").waitFor();
      await page.getByTestId("node-card").first().waitFor();
      await beat(page);
      await shot(page, "1-graph");
    });

    test("the list with the Frontier on", async ({ page }) => {
      await openAt(page, "browser", "j_launch", "list?frontier=1", fixed);
      await page.getByTestId("list-row").first().waitFor();
      await beat(page);
      await shot(page, "2-list-frontier");
    });

    test("the list as a tree, with its guide lines", async ({ page }) => {
      await openAt(page, "browser", "j_vendor_eval", "list", fixed);
      await page.getByTestId("list-fold-all").click();
      await page.locator('[data-testid="list-row"][data-container="true"]').first().evaluate((row) => { row.scrollIntoView({ block: "start" }); });
      await page.mouse.move(640, 20);
      await beat(page);
      await shot(page, "3-list-tree");
    });

    test("a node's context menu", async ({ page }) => {
      await openAt(page, "browser", "j_vendor_eval", "graph?detail=work", fixed);
      await nodeCard(page, "n_review_opens").waitFor();
      await beat(page);
      await nodeCard(page, "n_review_opens").click({ button: "right" });
      await page.getByTestId("context-menu").waitFor();
      await shot(page, "4-context-menu");
    });

    test("the walk through", async ({ page }) => {
      await openAt(page, "browser", "j_vendor_eval", "walk", fixed);
      await page.getByTestId("triage-card").waitFor();
      await beat(page);
      await shot(page, "5-walk-through");
    });

    test("the graph zoomed out", async ({ page }) => {
      await openAt(page, "browser", "j_vendor_eval", "graph?detail=all", fixed);
      await page.getByTestId("node-card").first().waitFor();
      await beat(page);
      await page.keyboard.press("f");
      await beat(page);
      await shot(page, "6-graph-zoomed-out");
    });
  });
}
