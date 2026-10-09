// The proof's media for the graph (briefs/proof/8.8/README.md): a screenshot of each ladder
// step, a node's trace, an edge's hover, the Signals lens, and the conditional and not-relevant
// cards, and a short video of expanding a stage in place and tracing a node, written to
// CAIRN_PROOF_OUT. Steps only wait for the state they picture; the e2e specs assert. Everything
// runs on the in-browser host, seeded on each load, at the fixed day the browser tests use.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=dist/proof npx playwright test -c playwright.proof.config.ts proof/graph.proof.ts)
import { join } from "node:path";

import { test, type Browser, type Page } from "@playwright/test";

import { lineBetween, pointOn } from "../e2e/canvas.ts";
import { nodeCard, openAt, openFromCanvas } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(900);
const fixed = { fixedToday: FIXED_TODAY };
const GRAPH = "plan/graph";

test.use({ viewport: { width: 1280, height: 780 } });

/** Opens the vendor evaluation's graph at `query`, once its first card is drawn. */
async function vendor(page: Page, query = ""): Promise<void> {
  await openAt(page, "browser", "j_vendor_eval", `${GRAPH}${query}`, fixed);
  await page.getByTestId("node-card").first().waitFor();
  await beat(page);
}

test("the ladder's four steps", async ({ page }) => {
  await vendor(page);
  await shot(page, "1-stages");
  for (const [step, name] of [["decisions", "2-decisions"], ["work", "3-work"], ["all", "4-all"]] as const) {
    await vendor(page, `?detail=${step}`);
    await shot(page, name);
  }
});

test("a stage expanded in place, a node's trace, and an edge's hover", async ({ page }) => {
  await vendor(page);
  // The view opens on the current stage; the whole graph is one key away.
  await page.keyboard.press("f");
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await nodeCard(page, "n_plan").waitFor();
  await beat(page);
  await shot(page, "5-expanded-in-place");
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await page.getByTestId("trace-bar").waitFor();
  await beat(page);
  await shot(page, "6-trace");
  const at = await pointOn(lineBetween(page, "n_access", "n_plan"));
  await page.mouse.move(at.x, at.y);
  await page.getByTestId("edge-hover").waitFor();
  await shot(page, "7-edge-hover");
});

test("the Signals lens, a settled not-relevant card, and a conditional one", async ({ page }) => {
  await vendor(page, "?detail=work&lens=gravity");
  await page.keyboard.press("f");
  await shot(page, "8-signals-gravity");
  await vendor(page, "?detail=work&show=notrelevant%2Cconditional");
  await page.keyboard.press("f");
  await shot(page, "9-not-relevant");
  await openAt(page, "browser", "j_hiring", `${GRAPH}?detail=all`, fixed);
  await page.getByTestId("node-card").first().waitFor();
  await beat(page);
  await page.keyboard.press("f");
  const panel = await openFromCanvas(page, "n_make_offer");
  await panel.getByTestId("actions").getByRole("button", { name: "Reopen" }).click();
  await nodeCard(page, "n_close_out").waitFor();
  await beat(page);
  await shot(page, "10-conditional");
});

/** The main flow: the graph at Stages, a stage expanded in place, a node traced. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 1280, height: 780 },
    recordVideo: { dir: join(out, "video"), size: { width: 1280, height: 780 } },
  });
  const page = await context.newPage();
  await vendor(page);
  // The view opens on the current stage; the whole graph is one key away.
  await page.keyboard.press("f");
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await nodeCard(page, "n_plan").waitFor();
  await beat(page);
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await page.getByTestId("trace-bar").waitFor();
  await beat(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
