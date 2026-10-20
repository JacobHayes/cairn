// Brief 5.7 in the browser, on the in-browser host: the one proposal-review flow, the scenario
// journey upgraded to the fixture's version 2 with each kind of conflict that version raises
// resolved and applied (B7, C14), where the review screen meets the canvas, the inspector and
// the host. What a proposal offers, blocks and reads as stale is model and host tests'.
import { expect, test } from "./test.ts";

import { openJourneyCard } from "./around.ts";
import { apply, blockers, editWhereVersionTwoChanges, pickNode, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits, scrollsOnce } from "./proposals.ts";
import { expectAddress, goTo, nodeCard, open, setDepth, syncChip } from "./shell.ts";

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
  // The layer opens at the page's own depth; the changes the proposal makes inside the stages are reached at Actions.
  await setDepth(page, "Actions");
  // The drafted proposal is also one click away in Recent, for when the screen that opened it is gone.
  await syncChip(page).click();
  await expect(page.getByTestId("sync-popover").getByTestId("sync-recent").filter({ hasText: "Drafted a proposal" }).getByRole("button", { name: "Review it" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sync-popover")).toHaveCount(0);
  // Still a conflict to decide: Apply is held, for that reason alone, and pressing it shows why.
  await expect.poll(() => blockers(page)).toEqual(["unresolved"]);
  await expect(page.getByTestId("apply-proposal")).toHaveAttribute("aria-disabled", "true");
  await expect(page.getByTestId("to-decide")).toBeVisible();
  await expect(nodeCard(page, "n_signoff")).toHaveAttribute("data-diff", "added");
  await expect(nodeCard(page, "n_signoff").getByTestId("card-diff")).toContainText("Added");
  await page.getByTestId("apply-proposal").click({ force: true });
  await expect(page.getByTestId("apply-waits")).toBeFocused();
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
  await goTo(page, "list");
  await expect(page.locator('[data-testid="list-row"][data-node="n_workload"]')).toHaveAttribute("data-diff", "removed");
  await expect(page.locator('[data-testid="list-row"][data-diff="removed"]')).toHaveCount(3);
  await saveEdits(page);
  await apply(page);
  await goTo(page, "graph");
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
  // Home waits on it too, in a fold after the journeys.
  await open(page, "browser", "/");
  await page.getByTestId("home-proposals").locator("summary").click();
  await page.getByTestId("home-proposals").getByTestId("proposal-row").getByRole("link").click();
  await reviewOpen(page);
  await expectAddress(page, reviewed);
  // Close leaves the layer for the journey itself, the view kept.
  await page.getByTestId("close-layer").click();
  await expect(page.getByTestId("layer-band")).toHaveCount(0);
  expect(new URL(page.url()).pathname).toBe("/journeys/j_vendor_eval");
});

test("C14, A11: saving a journey as a route is reviewed over the route's own page, in every view, and applied", async ({ page }) => {
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-save").click();
  await page.getByTestId("save-as-route-flow").getByLabel("Route id").fill("r_saved");
  await page.getByTestId("save-as-route-flow").getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  const layered = new URL(page.url()).pathname;
  expect(layered).toMatch(/^\/routes\/r_saved\/proposals\/pr_[0-9a-f]+$/);
  // The route's own views stay under the layer: a switch keeps the proposal in the address.
  // Every node of the route it creates is marked added, in the list and on the timeline alike.
  const added = { list: '[data-testid="route-row"][data-diff="added"]', timeline: '[data-diff="added"] [data-testid="timeline-words"]' };
  for (const to of ["list", "timeline"] as const) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${to}`).click();
    await expect(page.getByTestId("route-frame")).toHaveAttribute("data-screen", to);
    await expect(page.locator(added[to]).first()).toBeVisible();
    await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "reviewing");
    expect(new URL(page.url()).pathname).toBe(layered);
  }
  await apply(page);
  await page.getByTestId("close-layer").click();
  await expect(page.getByTestId("layer-band")).toHaveCount(0);
  expect(new URL(page.url()).pathname).toBe("/routes/r_saved");
});
