// The proof's media for brief 7.1 (briefs/proof/7.1/prove.sh): the walkthrough after the partner
// decision is answered, the same pass with every kind, and the pass after a node completed in
// its inspector, written to CAIRN_PROOF_OUT. It navigates and captures only; the journey runs
// on the in-browser host over a route like the vendor evaluation with one more decision.
import { join } from "node:path";

import { test, type Page } from "@playwright/test";

import { answerCard, publishFollowUpRoute } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { nodePanel } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const atCard = (page: Page, node: string) => page.locator(`[data-testid="triage-card"][data-node="${node}"]`).waitFor();
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1200, height: 760 } });

test("what an answer unlocked comes next, labeled", async ({ page }) => {
  const route = await publishFollowUpRoute(page);
  await startJourney(page, "browser", journeyName("Follow-up"), { route, version: 1 });
  await answerCard(page, "yes");
  await atCard(page, "n_partner_scope");
  await shot(page, "1-the-follow-up-comes-next");
  await page.getByTestId("chip-decisions").click();
  await atCard(page, "n_criteria");
  await shot(page, "2-every-kind-the-same-pass");
  await page.getByTestId("new-pass").click();
  await page.getByTestId("pass-rail").getByRole("link", { name: "Review the partner's criteria" }).click();
  await nodePanel(page, "n_criteria").getByTestId("actions").getByRole("button", { name: "Mark done" }).click();
  await page.getByTestId("back-to-pass").click();
  await atCard(page, "n_partner_results");
  await shot(page, "3-completed-from-the-pass");
});
