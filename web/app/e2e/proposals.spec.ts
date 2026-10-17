// Brief 5.7 in the browser, on the in-browser host: the one proposal-review flow, the scenario
// journey upgraded to the fixture's version 2 with each kind of conflict that version raises
// resolved and applied (B7, C14), where the review screen meets the canvas, the inspector and
// the host. What a proposal offers, blocks and reads as stale is model and host tests'.
import { expect, test } from "@playwright/test";

import { openJourneyCard } from "./around.ts";
import { blockers, confirmAndApply, editWhereVersionTwoChanges, pickNode, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits, scrollsOnce } from "./proposals.ts";
import { expectAddress, nodeCard, open, setDepth, syncChip } from "./shell.ts";

test("B7, C14: the scenario journey upgraded to version 2, each conflict resolved, then applied", async ({ page }) => {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-upgrade").click();
  const flow = page.getByTestId("upgrade-flow");
  await expect(flow.getByLabel("Upgrade to version")).toHaveValue("2");
  await flow.getByRole("button", { name: "Propose the upgrade" }).click();
  await page.setViewportSize({ width: 1440, height: 768 });
  await reviewOpen(page);
  // The drafted proposal is also one click away in Recent, for when the screen that opened it is gone.
  await syncChip(page).click();
  await expect(page.getByTestId("sync-popover").getByTestId("sync-recent").filter({ hasText: "Drafted a proposal" }).getByRole("button", { name: "Review it" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sync-popover")).toHaveCount(0);
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
  await setDepth(page, "Actions");
  await expect(nodeCard(page, "n_signoff")).toBeVisible();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Access to the environment");
  await expect(nodeCard(page, "n_workload")).toHaveCount(0);
});

test("C14, C16: a drafted upgrade is listed in its journey's Proposals tab and on Home, and opens under the journey", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-upgrade").click();
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  const reviewed = new URL(page.url()).pathname;
  expect(reviewed).toMatch(/^\/journeys\/j_vendor_eval\/proposals\/pr_[0-9a-f]+$/);
  // Away from it: the journey's tab names it, and its row opens the same address.
  await open(page, "browser", "/journeys/j_vendor_eval");
  await expect(page.getByTestId("tab-proposals")).toContainText("1");
  await page.getByTestId("tab-proposals").click();
  await page.getByTestId("proposals-pane").getByTestId("proposal-row").getByRole("link").click();
  await reviewOpen(page);
  await expectAddress(page, reviewed);
  // The address of the list opens the journey with the tab in view.
  await open(page, "browser", "/journeys/j_vendor_eval/proposals");
  await expect(page.getByTestId("tab-proposals")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByTestId("proposals-panel").getByTestId("proposal-row")).toHaveCount(1);
  // Home waits on it too.
  await open(page, "browser", "/");
  await page.getByTestId("home-proposals").getByTestId("proposal-row").getByRole("link").click();
  await reviewOpen(page);
  await expectAddress(page, reviewed);
  // The way back names the destination and its open proposals.
  await page.getByTestId("review-trail").getByRole("link", { name: "Open proposals" }).click();
  await expect(page.getByTestId("tab-proposals")).toHaveAttribute("aria-selected", "true");
});
