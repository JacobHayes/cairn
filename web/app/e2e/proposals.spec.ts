// Brief 5.7 in the browser, on the in-browser host: the scenario journey upgraded to the
// fixture's version 2 with each kind of conflict that version raises resolved and applied
// (B7, C14); a stale proposal refreshed and reviewed again before it applies (I6); the
// routeless journey saved as a route with a participation mapping and re-linked to it (B8,
// B9).
import { expect, test } from "@playwright/test";

import { chooseFromMenu, openRouteDetail, openJourneyCard, routeAction, startJourney } from "./around.ts";
import { blockers, confirmAndApply, editWhereVersionTwoChanges, openChanges, pickNode, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits, scrollsOnce } from "./proposals.ts";
import { state } from "./detail.ts";
import { goWithin, nodeCard, openFromCanvas, openJourney, rename } from "./shell.ts";

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

test("I6: a stale proposal shows what moved, and applies only once refreshed and reviewed again", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  await openJourneyCard(page, "browser", "j_vendor_eval");
  await chooseFromMenu(page, "journey-menu", "menu-upgrade");
  await page.getByTestId("upgrade-flow").getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);
  const address = page.url();
  await page.getByTestId("reviewed").check();
  await expect.poll(() => blockers(page)).toEqual([]);

  await openJourney(page, "browser", "j_vendor_eval");
  await rename(page, "n_findings", "Findings, drafted");
  await goWithin(page, address);
  const stale = page.getByTestId("stale");
  await expect(stale.getByTestId("intervening-patch")).toHaveCount(1);
  await expect.poll(() => blockers(page)).toContain("stale");

  await stale.getByTestId("refresh").click();
  await expect(page.getByTestId("stale")).toHaveCount(0);
  await expect(page.getByTestId("reviewed")).not.toBeChecked();
  await expect.poll(() => blockers(page)).toEqual(["unreviewed"]);
  await expect(page.getByTestId("apply-proposal")).toBeDisabled();
  await confirmAndApply(page);
});

test("B8, B9: the routeless journey saved as a route with a participation mapping, published, and re-linked to it", async ({ page }) => {
  await openJourneyCard(page, "browser", "j_bakeoff");
  await chooseFromMenu(page, "journey-menu", "menu-save");
  const save = page.getByTestId("save-as-route-flow");
  await save.getByLabel("Route id").fill("bake-off-route");
  await save.getByLabel("Route name").fill("Bake-off");
  await save.getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  // Each entity's mapping is decided at its node; the proposal card lists what is left until none is.
  const waiting = page.getByTestId("to-decide-item");
  await expect(waiting.first()).toBeVisible();
  await waiting.first().getByRole("button", { name: "Open" }).click();
  const undecided = page.locator('[data-testid="review-item"][data-item="participation"]').filter({ has: page.getByTestId("unresolved") });
  await scrollsOnce(page);
  const first = undecided.first();
  await first.getByLabel("New role title").fill("Trial lead");
  await first.getByRole("button", { name: "Map to a new role" }).click();
  let more = true;
  while (more) {
    for (const mapping of await undecided.all()) {
      await mapping.getByLabel("Map to").selectOption("drop");
    }
    await page.getByRole("link", { name: "Back to the proposal" }).click();
    more = (await waiting.count()) > 0;
    if (more) {
      await waiting.first().getByRole("button", { name: "Open" }).click();
    }
  }
  await expect(page.getByTestId("to-decide")).toHaveCount(0);
  await saveEdits(page);
  await confirmAndApply(page);

  await openRouteDetail(page, "browser", "bake-off-route");
  await routeAction(page, "Publish the draft");
  await openJourneyCard(page, "browser", "j_bakeoff");
  await chooseFromMenu(page, "journey-menu", "menu-relink");
  const relink = page.getByTestId("relink-flow");
  await relink.getByLabel("Re-link to route").selectOption("bake-off-route");
  await relink.getByRole("button", { name: "Propose the re-link" }).click();
  await reviewOpen(page);
  await confirmAndApply(page);
  await openJourneyCard(page, "browser", "j_bakeoff");
  await expect(page.getByTestId("overview-lineage")).toContainText("version 1");
});

test("C14: an editor follows its change when edits are dropped or another change is, an edit with a problem holds the apply, and a candidate that breaks a rule claims no diff", async ({ page }) => {
  const journey = await startJourney(page, "browser", "Edited", { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", journey);
  const kickoff = await openFromCanvas(page, "n_kickoff");
  await kickoff.getByRole("button", { name: "Mark reached" }).click();
  await expect(state(kickoff)).toHaveAttribute("data-status", "done");
  const workload = await openFromCanvas(page, "n_workload");
  await workload.getByTestId("break-down").click();
  const form = workload.getByTestId("break-down-form");
  await form.getByLabel("Piece title").fill("Ingest workload");
  await form.getByRole("button", { name: "Another piece" }).click();
  await form.getByLabel("Piece title").nth(1).fill("Query workload");
  await form.getByTestId("propose-breakdown").click();
  await reviewOpen(page);
  await openChanges(page);

  const added = page.locator('[data-testid="change"][data-op="add_node"]');
  await added.nth(1).getByRole("button", { name: "Edit" }).click();
  const second = added.nth(1).getByTestId("added-node-form").getByLabel("Title", { exact: true });
  await second.fill("Query workloads");
  await page.getByTestId("unsaved").getByRole("button", { name: "Drop them" }).click();
  await expect(second).toHaveValue("Query workload");

  await added.first().getByRole("button", { name: "Drop" }).click();
  await expect(added).toHaveCount(1);
  const title = added.first().getByTestId("added-node-form").getByLabel("Title", { exact: true });
  await expect(title).toHaveValue("Query workload");

  await title.fill("");
  await expect.poll(() => blockers(page)).toContain("editing");
  await expect(page.getByTestId("save-proposal")).toBeDisabled();
  await title.fill("Query workload");
  await expect.poll(() => blockers(page)).not.toContain("editing");

  const adding = page.getByTestId("proposal-add-node");
  await adding.getByLabel("New node title").fill("Query workload");
  await adding.getByRole("button", { name: "Add a node" }).click();
  await expect(added).toHaveCount(2);
  await added.nth(1).getByRole("button", { name: "Edit" }).click();
  await added.nth(1).getByTestId("added-node-form").getByLabel("Id", { exact: true }).fill("query-workload");
  await expect(page.getByTestId("no-graph-after")).toBeVisible();
  await expect(page.locator('[data-testid="proposal-violations"] [data-code="duplicate_sibling_id"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="node-card"][data-trace="Remove"]')).toHaveCount(0);
  await expect(page.getByTestId("frontier-after")).toHaveCount(0);
});

test("I6: a stale route proposal lists each record that moved since it was drafted", async ({ page }) => {
  await openJourneyCard(page, "browser", "j_bakeoff");
  await chooseFromMenu(page, "journey-menu", "menu-save");
  const save = page.getByTestId("save-as-route-flow");
  await save.getByLabel("Route id").fill("hiring-loop");
  await save.getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  const address = page.url();
  await openRouteDetail(page, "browser", "hiring-loop");
  await routeAction(page, "Open a draft");
  await goWithin(page, address);
  await expect(page.getByTestId("stale").getByTestId("intervening-record").first()).toBeVisible();
  await expect.poll(() => blockers(page)).toContain("stale");
});
