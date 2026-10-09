// The proof's pictures for brief 5.8 (briefs/proof/5.8/prove.sh): the assistant panel on the
// server host against 4.4's scripted provider. A screenshot of each acceptance state (the
// panel opened, a turn in progress, a direct change reported with its node, a breakdown
// drafted as a proposal, that proposal in review and applied, the conversation read back on
// the overview, a route's draft, the in-browser host without the panel, dark and narrow) and
// a short video of the main flow, written to CAIRN_PROOF_OUT. Each step asserts what its
// picture is meant to show, so a picture of the wrong state fails the run.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { startVendorJourney } from "../e2e/acting.ts";
import { ask, openPanel, patchId, script } from "../e2e/assistant.ts";
import { openJourneyCard } from "../e2e/around.ts";
import { state } from "../e2e/detail.ts";
import { confirmAndApply, reviewOpen } from "../e2e/proposals.ts";
import { nodePanel, open, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
/** A picture of the window as a person sees it. */
async function shot(page: Page, name: string): Promise<void> {
  await page.screenshot({ path: join(out, `${name}.png`) });
}
const beat = (page: Page) => page.waitForTimeout(700);

test.use({ viewport: { width: 1440, height: 900 } });
test.describe.configure({ mode: "serial" });

/** The two turns: a direct change, then a breakdown drafted as a proposal; pictures when asked. */
async function converse(page: Page, journey: string, pictures: boolean): Promise<void> {
  await openJourney(page, "server", journey);
  const panel = await openPanel(page);
  if (pictures) {
    await shot(page, "1-panel-opened");
  }
  await script(page, [
    {
      calls: [{ name: "transition_node", arguments: { journey, node: "n_kickoff", transition: "reach", patch_id: patchId(), base_revision: 1 } }],
      after_ms: pictures ? 1500 : 0,
    },
    { say: "Done: **Kickoff** is reached, which opens setup. The access request and the test plan are next." },
  ]);
  await panel.getByTestId("assistant-message").fill("Kickoff happened this morning.");
  await panel.getByTestId("assistant-send").click();
  if (pictures) {
    await expect(panel.getByTestId("assistant-working")).toBeVisible();
    await shot(page, "2-working");
  }
  await expect(panel).toHaveAttribute("data-status", "idle");
  await expect(panel.getByTestId("assistant-applied")).toHaveCount(1);
  if (pictures) {
    await shot(page, "3-direct-change-reported");
  }
  await beat(page);
  await panel.locator('[data-testid="assistant-node"][data-node="n_kickoff"]').click();
  await expect(state(nodePanel(page, "n_kickoff"))).toHaveAttribute("data-status", "done");
  await beat(page);

  const pieces = ["Ingest workload", "Query workload"].map((title, index) => ({
    op: "add_node",
    node: { key: `n_piece_${String(index)}`, id: `piece-${String(index)}`, kind: "deliverable", title, parent: "n_workload" },
  }));
  await script(page, [
    { calls: [{ name: "apply_patch", arguments: { patch: { id: patchId(), target: { journey }, base_revision: 2, mutations: pieces } } }] },
    { say: "I drafted the breakdown as a proposal: two deliverables under the test workload. Review it and apply it when it looks right." },
  ]);
  await ask(page, "Break the test workload down into an ingest workload and a query workload.");
  await expect(panel.getByTestId("assistant-proposed")).toHaveAttribute("data-because", "structural");
  if (pictures) {
    await shot(page, "4-breakdown-proposed");
  }
  await beat(page);
}

/** The proposal opened from the panel, reviewed, and applied; back on the journey. */
async function reviewAndApply(page: Page, pictures: boolean): Promise<void> {
  await page.getByTestId("assistant-panel").getByTestId("review-proposal").click();
  await reviewOpen(page);
  await expect(page.locator('[data-testid="diff-node"][data-status="added"]')).toHaveCount(2);
  if (pictures) {
    await shot(page, "5-proposal-in-review");
  }
  await beat(page);
  await confirmAndApply(page);
  await beat(page);
  await page.getByTestId("applied-journey").click();
  await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]')).toHaveCount(2);
  await expect(page.getByTestId("assistant-panel").getByTestId("review-proposal")).toHaveCount(1);
  if (pictures) {
    await shot(page, "6-applied-on-the-canvas");
  }
}

test("I5, I7: a direct change and a proposal, reviewed and applied", async ({ page }) => {
  const journey = await startVendorJourney(page);
  await converse(page, journey, true);
  await reviewAndApply(page, true);
  await openJourneyCard(page, "server", journey);
  await page.reload();
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveCount(2);
  await shot(page, "7-journey-page-reads-it-back");
});

test("a route's draft has its own conversation", async ({ page }) => {
  await open(page, "server", "/routes/product-launch");
  const panel = await openPanel(page);
  await script(page, [{ say: "No draft is open yet. Tell me what to add and I will draft it as a proposal on a new draft." }]);
  await ask(page, "How would you extend this route?");
  await expect(panel).toHaveAttribute("data-target", "route:product-launch");
  await shot(page, "8-route-draft");
});

test("the in-browser host has no panel", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("assistant-toggle")).toHaveCount(0);
  await shot(page, "9-in-browser-host-without-it");
});

test("the dark theme and a narrow screen", async ({ browser, baseURL }) => {
  const dark = await browser.newPage({ baseURL: baseURL ?? "", colorScheme: "dark", viewport: { width: 1440, height: 900 } });
  await converse(dark, await startVendorJourney(dark), false);
  await shot(dark, "10-dark-theme");
  await dark.close();
  const narrow = await browser.newPage({ baseURL: baseURL ?? "", viewport: { width: 390, height: 844 } });
  await converse(narrow, await startVendorJourney(narrow), false);
  expect(await narrow.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(narrow, "11-narrow-screen");
  await narrow.close();
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1440, height: 900 }, recordVideo: { dir: join(out, "video"), size: { width: 960, height: 600 } } });
  const page = await context.newPage();
  await converse(page, await startVendorJourney(page), false);
  await reviewAndApply(page, false);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
