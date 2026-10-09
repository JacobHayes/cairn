// Brief 5.7 in the browser, on the in-browser host: the scenario journey upgraded to the
// fixture's version 2 with each kind of conflict that version raises resolved and applied
// (B7, C14); a stale proposal refreshed and reviewed again before it applies (I6); the
// routeless journey saved as a route with a participation mapping and re-linked to it (B8,
// B9); and the placeholder broken down from triage through a proposal (B10).
import { expect, test } from "@playwright/test";

import { openRouteDetail, openOverview, routeAction, startJourney } from "./around.ts";
import { card, openActing, startVendorJourney } from "./acting.ts";
import { blockers, confirmAndApply, editWhereVersionTwoChanges, publishVersionTwo, resolve, reviewItem, reviewOpen, saveEdits } from "./proposals.ts";
import { section, state } from "./detail.ts";
import { goWithin, nodeCard, openFromCanvas, openJourney, rename } from "./shell.ts";

test("B7, C14: the scenario journey upgraded to version 2, each conflict resolved, then applied", async ({ page }) => {
  await editWhereVersionTwoChanges(page, "browser");
  await publishVersionTwo(page, "browser");
  await openOverview(page, "browser", "j_vendor_eval");
  const flow = page.getByTestId("upgrade-flow");
  await expect(flow.getByLabel("Upgrade to version")).toHaveValue("2");
  await flow.getByRole("button", { name: "Propose the upgrade" }).click();
  await reviewOpen(page);

  await expect(reviewItem(page, "conflict", "n_access")).toHaveAttribute("data-about", "field");
  await expect(reviewItem(page, "conflict", "n_baseline")).toHaveAttribute("data-about", "field");
  await expect(reviewItem(page, "kept_local_edit", "n_kickoff")).toBeVisible();
  await expect(reviewItem(page, "orphan", "n_workload")).toBeVisible();
  await expect(page.locator('[data-testid="diff-node"][data-node="n_signoff"]')).toHaveAttribute("data-status", "added");
  await expect(page.locator('[data-testid="node-card"][data-node="n_signoff"]')).toHaveAttribute("data-trace", "added");
  await expect.poll(() => blockers(page)).toEqual(expect.arrayContaining(["unresolved", "unreviewed"]));

  await resolve(page, "n_access", "keep_journey");
  await resolve(page, "n_baseline", "take_route");
  const orphan = reviewItem(page, "orphan", "n_workload");
  await orphan.getByTestId("orphan-remove").check();
  await expect(orphan.locator('[data-testid="removal-descendant"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="diff-node"][data-node="n_workload"]')).toHaveAttribute("data-status", "removed");
  await saveEdits(page);
  await confirmAndApply(page);

  await page.getByTestId("applied-journey").click();
  await expect(nodeCard(page, "n_signoff")).toBeVisible();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Access to the environment");
  await expect(nodeCard(page, "n_workload")).toHaveCount(0);
});

test("I6: a stale proposal shows what moved, and applies only once refreshed and reviewed again", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  await openOverview(page, "browser", "j_vendor_eval");
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
  await openOverview(page, "browser", "j_bakeoff");
  const save = page.getByTestId("save-as-route-flow");
  await save.getByLabel("Route id").fill("bake-off-route");
  await save.getByLabel("Route name").fill("Bake-off");
  await save.getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  const mappings = page.locator('[data-testid="review-item"][data-item="participation"]');
  await expect(mappings.first()).toBeVisible();
  const first = mappings.first();
  await first.getByLabel("New role title").fill("Trial lead");
  await first.getByRole("button", { name: "Map to a new role" }).click();
  for (const mapping of (await mappings.all()).slice(1)) {
    await mapping.getByLabel("Map to").selectOption("drop");
  }
  await expect(page.getByTestId("unresolved")).toHaveCount(0);
  await saveEdits(page);
  await confirmAndApply(page);

  await openRouteDetail(page, "browser", "bake-off-route");
  await routeAction(page, "Publish the draft");
  await openOverview(page, "browser", "j_bakeoff");
  const relink = page.getByTestId("relink-flow");
  await relink.getByLabel("Re-link to route").selectOption("bake-off-route");
  await relink.getByRole("button", { name: "Propose the re-link" }).click();
  await reviewOpen(page);
  await confirmAndApply(page);
  await openOverview(page, "browser", "j_bakeoff");
  await expect(page.getByTestId("overview-lineage")).toContainText("version 1");
});

test("B10: the placeholder broken down from its triage card through a proposal", async ({ page }) => {
  const journey = await startJourney(page, "browser", "Broken down", { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", journey);
  const kickoff = await openFromCanvas(page, "n_kickoff");
  await kickoff.getByRole("button", { name: "Mark reached" }).click();
  await expect(state(kickoff)).toHaveAttribute("data-status", "done");
  await openActing(page, "browser", journey, "triage?kind=deliverable");
  for (let pass = 0; pass < 4 && (await card(page).getAttribute("data-node")) !== "n_workload"; pass += 1) {
    await card(page).getByTestId("pass").click();
  }
  await expect(card(page)).toHaveAttribute("data-node", "n_workload");
  await expect(card(page).getByTestId("acts")).toHaveAttribute("data-acts", /breakdown/);
  await card(page).getByTestId("break-down").click();
  const form = card(page).getByTestId("break-down-form");
  await form.getByLabel("Piece title").fill("Ingest workload");
  await form.getByRole("button", { name: "Another piece" }).click();
  await form.getByLabel("Piece title").nth(1).fill("Query workload");
  await form.getByTestId("propose-breakdown").click();
  await reviewOpen(page);
  await expect(page.locator('[data-testid="diff-node"][data-status="added"]')).toHaveCount(2);
  await confirmAndApply(page);
  await page.getByTestId("applied-journey").click();
  for (const title of ["Ingest workload", "Query workload"]) {
    await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]').filter({ hasText: title })).toHaveCount(1);
  }
});

test("B10, I6 on the server host: a placeholder broken down from its node detail through a proposal", { tag: "@server" }, async ({ page }) => {
  const journey = await startVendorJourney(page);
  const document = (await (await page.request.get(`/api/journeys/${journey}/document`)).json()) as { journey: { revision: number } };
  const reached = await page.request.post(`/api/journeys/${journey}/patches`, {
    data: {
      patch: {
        id: `p_${crypto.randomUUID().replaceAll("-", "")}`,
        target: { journey },
        base_revision: document.journey.revision,
        mutations: [{ op: "transition", node: "n_kickoff", transition: "reach" }],
      },
    },
  });
  expect(reached.ok(), await reached.text()).toBe(true);
  await openJourney(page, "server", journey);
  const workload = await section(await openFromCanvas(page, "n_workload"), "blocking");
  await workload.getByTestId("break-down").click();
  await workload.getByTestId("break-down-form").getByLabel("Piece title").fill("Ingest workload");
  await workload.getByTestId("propose-breakdown").click();
  await reviewOpen(page);
  await expect(page.locator('[data-testid="frontier-node"][data-status="new"]')).not.toHaveCount(0);
  await confirmAndApply(page);
  await page.getByTestId("applied-journey").click();
  await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]').filter({ hasText: "Ingest workload" })).toHaveCount(1);
});

test("C14: an editor follows its change when edits are dropped or another change is, an edit with a problem holds the apply, and a candidate that breaks a rule claims no diff", async ({ page }) => {
  const journey = await startJourney(page, "browser", "Edited", { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", journey);
  const kickoff = await openFromCanvas(page, "n_kickoff");
  await kickoff.getByRole("button", { name: "Mark reached" }).click();
  await expect(state(kickoff)).toHaveAttribute("data-status", "done");
  const workload = await section(await openFromCanvas(page, "n_workload"), "blocking");
  await workload.getByTestId("break-down").click();
  const form = workload.getByTestId("break-down-form");
  await form.getByLabel("Piece title").fill("Ingest workload");
  await form.getByRole("button", { name: "Another piece" }).click();
  await form.getByLabel("Piece title").nth(1).fill("Query workload");
  await form.getByTestId("propose-breakdown").click();
  await reviewOpen(page);

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
  await expect(page.getByTestId("diff-unknown")).toBeVisible();
  await expect(page.locator('[data-testid="proposal-violations"] [data-code="duplicate_sibling_id"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="diff-node"][data-status="removed"]')).toHaveCount(0);
  await expect(page.getByTestId("frontier-after")).toHaveCount(0);
});

test("I6: a stale route proposal lists each record that moved since it was drafted", async ({ page }) => {
  await openOverview(page, "browser", "j_bakeoff");
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
