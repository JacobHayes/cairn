// Brief 5.7 in the browser, on the in-browser host: the one proposal-review flow, the scenario
// journey upgraded to the fixture's version 2 with each kind of conflict that version raises
// resolved and applied (B7, C14), where the review screen meets the canvas, the inspector and
// the host. What a proposal offers, blocks and reads as stale is model and host tests'.
import { expect, test } from "@playwright/test";

import { chooseFromMenu, openJourneyCard } from "./around.ts";
import { blockers, confirmAndApply, editWhereVersionTwoChanges, pickNode, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits, scrollsOnce } from "./proposals.ts";
import { nodeCard } from "./shell.ts";

test("B7, C14: the scenario journey upgraded to version 2, each conflict resolved, then applied", async ({ page }) => {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await chooseFromMenu(page, "journey-menu", "menu-upgrade");
  const flow = page.getByTestId("upgrade-flow");
  await expect(flow.getByLabel("Upgrade to version")).toHaveValue("2");
  await flow.getByRole("button", { name: "Propose the upgrade" }).click();
  await page.setViewportSize({ width: 1440, height: 768 });
  await reviewOpen(page);
  // Reviewed, and still a conflict to decide: Apply stays off, for that reason alone.
  await page.getByTestId("reviewed").check();
  await expect.poll(() => blockers(page)).toEqual(["unresolved"]);
  await expect(page.getByTestId("apply-proposal")).toBeDisabled();
  await expect(page.getByTestId("review-conflicts")).toBeVisible();
  await expect(page.getByTestId("node-card").and(page.locator('[data-node="n_signoff"]'))).toHaveAttribute("data-trace", "Add");
  await pickNode(page, "n_access");
  await expect(reviewItem(page, "conflict", "n_access")).toHaveAttribute("data-about", "field");
  await scrollsOnce(page);
  await pickNode(page, "n_baseline");
  await expect(reviewItem(page, "conflict", "n_baseline")).toHaveAttribute("data-about", "field");
  await pickNode(page, "n_kickoff");
  await expect(reviewItem(page, "kept_local_edit", "n_kickoff")).toBeVisible();
  await pickNode(page, "n_workload");
  await expect(reviewItem(page, "orphan", "n_workload")).toBeVisible();

  await resolve(page, "n_access", "keep_journey");
  await resolve(page, "n_baseline", "take_route");
  await pickNode(page, "n_workload");
  const orphan = reviewItem(page, "orphan", "n_workload");
  await orphan.getByTestId("orphan-remove").check();
  await expect(orphan.locator('[data-testid="removal-descendant"]')).toHaveCount(2);
  await page.getByTestId("projection-list").click();
  await expect(page.locator('[data-testid="diff-node"][data-node="n_workload"]')).toHaveAttribute("data-status", "remove");
  await expect(page.locator('[data-testid="diff-node"][data-status="remove"]')).toHaveCount(3);
  await saveEdits(page);
  await confirmAndApply(page);

  await page.getByTestId("applied-journey").click();
  await page.getByTestId("ladder").getByText("All", { exact: true }).click();
  await expect(nodeCard(page, "n_signoff")).toBeVisible();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Access to the environment");
  await expect(nodeCard(page, "n_workload")).toHaveCount(0);
});
