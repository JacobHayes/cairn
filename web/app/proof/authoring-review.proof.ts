// The proof's pictures for brief 8.11 (briefs/proof/8.11/README.md): a route draft in the frame
// with its draft card and a node's form, a journey's Edit structure, and a proposal under review
// as the Graph and as the List, light, 1440px, on the in-browser host. Steps only navigate and
// wait for the state they picture; the e2e specs assert.
//
// usage: (cd web/app && CAIRN_PROOF_OUT=../../briefs/proof/8.11 npx playwright test -c playwright.proof.config.ts proof/authoring-review.proof.ts)
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { chooseFromMenu, openJourneyCard, openRouteDetail, routeAction } from "../e2e/around.ts";
import { addNode, newRoute, openEditing } from "../e2e/authoring.ts";
import { editWhereVersionTwoChanges, pickNode, publishVersionTwo, reviewOpen } from "../e2e/proposals.ts";
import { goWithin, nodeCard } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
test.use({ viewport: { width: 1440, height: 900 } });

async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(900);
  await page.screenshot({ path: join(out, `${name}.png`) });
}

test("a route draft: its card, a node's form, and the list", async ({ page }) => {
  await openRouteDetail(page, "browser", "vendor-evaluation");
  await routeAction(page, "Open a draft");
  await goWithin(page, "/routes/vendor-evaluation/draft");
  await page.getByTestId("node-card").first().waitFor();
  // A deliverable with no chain to the final milestone: its card and the draft card say so.
  await addNode(page, "deliverable", "Order the test data");
  await goWithin(page, "/routes/vendor-evaluation/draft");
  await expect(page.getByTestId("route-notice").first()).toBeVisible();
  await shot(page, "route-draft");
  // Drilled into Setup, the cards are near: the notice's chip and a card's date rule in words.
  await goWithin(page, "/routes/vendor-evaluation/draft?in=n_setup");
  await expect(page.getByTestId("card-notice").first()).toBeVisible();
  await shot(page, "route-cards");
  await goWithin(page, "/routes/vendor-evaluation/draft/nodes/n_partner_runs");
  await expect(page.getByTestId("node-form")).toBeVisible();
  await shot(page, "node-form");
  await goWithin(page, "/routes/vendor-evaluation/draft?view=list");
  await expect(page.getByTestId("route-list")).toBeVisible();
  await shot(page, "route-list");
});

test("a new route's empty draft", async ({ page }) => {
  await newRoute(page, "browser", "Supplier review");
  await expect(page.getByTestId("draft-card")).toBeVisible();
  await shot(page, "draft-empty");
});

test("Edit structure on a journey", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await page.getByTestId("node-card").first().waitFor();
  await nodeCard(page, "n_access").click({ modifiers: ["Shift"] });
  await nodeCard(page, "n_plan").click({ modifiers: ["Shift"] });
  await expect(page.getByTestId("bulk-bar")).toBeVisible();
  await shot(page, "edit-structure");
});

test("a proposal under review, as the Graph and as the List", async ({ page }) => {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await chooseFromMenu(page, "journey-menu", "menu-upgrade");
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  await shot(page, "review-graph");
  await pickNode(page, "n_access");
  await shot(page, "review-item-editor");
  await page.getByTestId("projection-list").click();
  await expect(page.getByTestId("diff-list")).toBeVisible();
  await shot(page, "review-list");
});
