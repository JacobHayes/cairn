// The proof's media for the answer rationale (briefs/proof/7.2/prove.sh): a screenshot of each
// state, written to CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show, so a
// picture of the wrong state fails the run. It runs on the in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { card, openActing } from "../e2e/acting.ts";
import { journeyName, startJourney } from "../e2e/around.ts";
import { dismissNotices } from "../e2e/authoring.ts";
import { nodePanel } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });

test.use({ viewport: { width: 1280, height: 800 } });

test("an answer with a reason, read back, then revised with none", async ({ page }) => {
  const journey = await startJourney(page, "browser", journeyName("Rationale"), { route: "vendor-evaluation", version: 1 });
  await openActing(page, "browser", journey, "next/cards?decisions=1");
  await expect(card(page)).toHaveAttribute("data-node", "n_partner_runs");
  await card(page).getByRole("button", { name: "Answer", exact: true }).click();
  await card(page).getByLabel("Answer").selectOption("yes");
  await card(page).getByLabel("Why").fill("- a partner brings the **domain**\n- see [their notes](https://example.org/notes)");
  await shot(page, "1-answer-with-a-reason");
  await card(page).getByRole("button", { name: "Save the answer" }).click();
  await expect(card(page)).not.toHaveAttribute("data-node", "n_partner_runs");

  await openActing(page, "browser", journey, "plan/graph/nodes/n_partner_runs?decisions=1");
  const panel = nodePanel(page, "n_partner_runs");
  await expect(panel.getByTestId("rationale").locator("li")).toHaveCount(2);
  await dismissNotices(page);
  await page.getByTestId("decision-table").scrollIntoViewIfNeeded();
  await shot(page, "2-decision-view-and-detail");

  await panel.getByRole("button", { name: "Revise the answer" }).click();
  await expect(panel.getByLabel("Why")).toHaveValue("");
  await expect(panel.getByTestId("answer-why")).toContainText("Previous reason");
  await panel.getByTestId("answer-editor").getByLabel("Answer").selectOption("no");
  await shot(page, "3-a-new-answer-starts-empty");
  await panel.getByRole("button", { name: "Save the answer" }).click();
  await expect(panel.getByTestId("rationale")).toHaveCount(0);
  await dismissNotices(page);
  await panel.getByTestId("history").locator("summary").click();
  await expect(panel.getByTestId("history-answer")).toHaveCount(2);
  await panel.getByTestId("history").scrollIntoViewIfNeeded();
  await shot(page, "4-history-keeps-each-reason");
});
