// Brief 5.5 on the in-browser host, seeded from the fixtures on each load: a journey started
// from a route lands in its walkthrough (B1); a journey's life through complete, archive, and
// a hard delete behind its typed name (B11, A19); a fixture's route file imported and exported
// identically (A13); a route with three versions, journeys on each, still offering upgrades
// once retired (C17, A19); two entities merged with history intact (E6); the index's filters
// and the cross-journey "mine" list (C16, E4); and who the local user is (H3).
import { expect, test } from "@playwright/test";

import {
  addEntity,
  fixtureRoute,
  journeyName,
  mergeEntities,
  openOverview,
  openRouteDetail,
  routeAction,
  routeRevision,
  savedText,
  setStatus,
  startJourney,
  versionJourneys,
} from "./around.ts";
import { nodeCard, nodePanel, open, openFromCanvas, openJourney } from "./shell.ts";
import { section } from "./detail.ts";

test("B1: a journey started from the fixture route lands in its walkthrough, and the index lists it", async ({ page }) => {
  const name = journeyName("Evaluation");
  const id = await startJourney(page, "browser", name, { route: "vendor-evaluation" });
  await expect(page.getByTestId("triage-card")).toBeVisible();
  await expect(page.getByTestId("triage-card")).toHaveAttribute("data-kind", "decision");
  await page.getByRole("link", { name: "Journeys" }).first().click();
  const row = page.locator(`[data-testid="journey-row"][data-journey="${id}"]`);
  await expect(row).toContainText(name);
  await expect(row.getByTestId("row-lineage")).toContainText("version 1");
  const empty = await startJourney(page, "browser", journeyName("Ad hoc"));
  await openOverview(page, "browser", empty);
  await expect(page.getByTestId("overview-lineage")).toContainText("Started empty");
});

test("B11, A19: completing leaves the active index and mine; archiving, then deleting behind the typed name", async ({ page }) => {
  await open(page, "browser", "/mine");
  await expect(page.locator('[data-testid="mine-journey"][data-journey="j_bakeoff"]')).toBeVisible();
  await openOverview(page, "browser", "j_bakeoff");
  await setStatus(page, "completed");
  await open(page, "browser", "/");
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(page.locator('[data-testid="journey-row"][data-journey="j_bakeoff"]')).toHaveCount(0);
  await open(page, "browser", "/?status=completed");
  await expect(page.locator('[data-testid="journey-row"][data-journey="j_bakeoff"]')).toBeVisible();
  await open(page, "browser", "/mine");
  await expect(page.getByTestId("mine-journey").first()).toBeVisible();
  await expect(page.locator('[data-testid="mine-journey"][data-journey="j_bakeoff"]')).toHaveCount(0);
  await openOverview(page, "browser", "j_bakeoff");
  await setStatus(page, "archived");
  await expect(page.getByTestId("header-editor")).toHaveCount(0);
  const remove = page.getByTestId("delete-journey");
  const confirm = remove.getByRole("button", { name: "Delete the journey" });
  await remove.getByRole("textbox").fill("Not its name");
  await expect(confirm).toBeDisabled();
  const name = (await page.getByTestId("journey-name").textContent()) ?? "";
  await remove.getByRole("textbox").fill(name);
  await confirm.click();
  await expect(page).toHaveURL(/\/\?status=any$/);
  await expect(page.getByTestId("journey-row").first()).toBeVisible();
  await expect(page.locator('[data-testid="journey-row"][data-journey="j_bakeoff"]')).toHaveCount(0);
});

test("A13, A11: the fixture's route file imported as a draft exports identically to the version it was published as", async ({ page }) => {
  await openRouteDetail(page, "browser", "hiring-loop");
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles(fixtureRoute("hiring-loop"));
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  const draft = await savedText(page, () => page.getByTestId("route-actions").getByRole("button", { name: "Export the draft" }).click());
  const version = await savedText(page, () => page.locator('[data-testid="version"][data-version="1"]').getByRole("button", { name: "Export" }).click());
  expect(draft).toBe(version);
  expect(draft).toContain("route: hiring-loop");
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles(fixtureRoute("hiring-loop"));
  await expect(page.locator('[data-testid="violation"][data-code="draft_exists"]')).toBeVisible();
  const before = await routeRevision(page);
  await page.getByRole("button", { name: "Discard the open draft and import" }).click();
  await expect(page.getByTestId("refused")).toHaveCount(0);
  await expect.poll(() => routeRevision(page), "the discard and the import, a patch each").toBe(before + 2);
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  await routeAction(page, "Discard the draft");
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "none");
});

test("C17, A19: three versions with journeys on each; retired, the route still offers its journeys upgrades", async ({ page }) => {
  await openRouteDetail(page, "browser", "vendor-evaluation");
  await routeAction(page, "Open a draft");
  await routeAction(page, "Publish the draft");
  const second = await startJourney(page, "browser", journeyName("On two"), { route: "vendor-evaluation", version: 2 });
  await openRouteDetail(page, "browser", "vendor-evaluation");
  await routeAction(page, "Open a draft");
  await routeAction(page, "Publish the draft");
  const third = await startJourney(page, "browser", journeyName("On three"), { route: "vendor-evaluation", version: 3 });
  await openRouteDetail(page, "browser", "vendor-evaluation");
  await routeAction(page, "Retire");
  await expect(page.getByTestId("route-detail").getByTestId("retired")).toBeVisible();
  await expect.poll(() => versionJourneys(page, 1)).toEqual({ j_vendor_eval: "available" });
  await expect.poll(() => versionJourneys(page, 2)).toEqual({ [second]: "available" });
  await expect.poll(() => versionJourneys(page, 3)).toEqual({ [third]: "none" });
  await expect(page.getByTestId("start-from-version")).toHaveCount(0);
  await open(page, "browser", "/?route=vendor-evaluation&upgrade=1");
  await expect(page.locator('[data-testid="journey-row"]')).toHaveCount(2);
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${second}"] [data-testid="upgrade"]`)).toBeVisible();
  await open(page, "browser", "/new?route=vendor-evaluation");
  await page.getByTestId("new-journey-form").getByLabel("Name").fill(journeyName("Not from a retired route"));
  await expect(page.getByTestId("route-unavailable")).toBeVisible();
  await expect(page.getByRole("button", { name: "Start the journey" })).toBeDisabled();
  await page.getByLabel("Start from").selectOption("hiring-loop");
  await expect(page.getByTestId("route-unavailable")).toHaveCount(0);
  await expect(page.getByLabel("Start from").locator('option[value="vendor-evaluation"]')).toHaveCount(0);
});

test("E6: two entities merged, the journey reads the survivor and its history is intact", async ({ page }) => {
  const panel = await (async () => {
    await openJourney(page, "browser", "j_vendor_eval");
    return openFromCanvas(page, "n_who_owns");
  })();
  const history = await section(panel, "history");
  await expect(history.getByTestId("history-patch").first()).toBeVisible();
  const before = await history.getByTestId("history-patch").count();
  await open(page, "browser", "/entities");
  const director = await addEntity(page, "Evaluation Director");
  await mergeEntities(page, director, "e_lead");
  await expect(page.locator(`[data-testid="entity"][data-entity="${director}"]`).getByTestId("entity-emails")).toContainText("lead@example.org");
  await expect(page.locator('[data-testid="entity"][data-entity="e_lead"]')).toHaveCount(0);
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(nodeCard(page, "n_plan").getByTestId("card-owner")).toContainText("Evaluation Director");
  const after = await section(await openFromCanvas(page, "n_who_owns"), "history");
  await expect(after.getByTestId("history-patch")).toHaveCount(before);
});

test("H3: the local user, its identity, and its entities, offered for merging; the index's mine filter", async ({ page }) => {
  await open(page, "browser", "/me");
  await expect(page.getByTestId("user")).toHaveText("u_local");
  await expect(page.getByTestId("identity")).toHaveCount(1);
  await expect(page.getByTestId("merge-offer")).toBeVisible();
  expect(await page.getByTestId("your-entity").count(), "the offer is for duplicates").toBeGreaterThan(1);
  await expect(page.getByTestId("link-identity")).toHaveCount(0);
  await page.getByTestId("merge-offer").getByRole("link", { name: "Merge them" }).click();
  await expect(page.getByTestId("merge-entities").getByLabel("Keep")).not.toHaveValue("");
  await open(page, "browser", "/?mine=1");
  await expect(page.locator('[data-testid="journey-row"][data-journey="j_vendor_eval"]')).toContainText("mine");
});

test("B3: an entity answer names a new person, made in the same patch, who joins the deployment's entities", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  const panel = await openFromCanvas(page, "n_findings_reviewer");
  await panel.getByRole("button", { name: "Revise the answer" }).click();
  await panel.getByLabel("Or a new entity").fill("Not sent");
  await panel.getByRole("button", { name: "Cancel" }).click();
  await panel.getByRole("button", { name: "Revise the answer" }).click();
  await expect(panel.getByLabel("Or a new entity")).toHaveValue("");
  const person = journeyName("Outside Reviewer");
  await panel.getByLabel("Or a new entity").fill(person);
  await page.reload();
  await expect(nodePanel(page, "n_findings_reviewer").getByLabel("Or a new entity")).toHaveValue(person);
  await nodePanel(page, "n_findings_reviewer").getByRole("button", { name: "Save the answer" }).click();
  await expect(nodePanel(page, "n_findings_reviewer").getByTestId("answer")).toContainText(person);
  await open(page, "browser", "/entities");
  await expect(page.getByTestId("entity-name").getByText(person, { exact: true })).toBeVisible();
});
