// Brief 5.6 on the in-browser host, seeded from the fixtures on each load: a small route
// authored by hand from an empty draft, published, exported, and imported back to the same
// graph (A12); edges drawn on the canvas, an ancestor refused with the reason (A3); three
// violations shown at three fields (A15); a kind's form with its own fields (A1a); and on a
// fixture journey, a local node added, a route-copied title edited and reset, a node removed
// with its cascade and restored from its tombstone (B4, A18), a rename that leaves a condition
// intact (Identity and references), and a breakdown by hand (B10).
import { expect, test, type Page } from "@playwright/test";

import { savedText } from "./around.ts";
import { addNode, addRole, formField, newRoute, nodeForm, openEditing, openForm, pickByTitle, routeName, saveForm, section, structure } from "./authoring.ts";
import { menuItem, nodeCard, open, openFromCanvas, syncChip } from "./shell.ts";

/** The fields the open node's form offers. */
async function offered(page: Page): Promise<string[]> {
  return nodeForm(page).getByTestId("author-field").evaluateAll((fields) => fields.map((field) => field.getAttribute("data-field") ?? ""));
}

/** Opens a draft node's structure from its card. */
async function openNode(page: Page, key: string): Promise<void> {
  await nodeCard(page, key).getByTestId("card-open").click();
  await expect(structure(page)).toHaveAttribute("data-node", key);
}

/** Authors the small route the acceptance names, from its empty draft; the keys by title. */
async function authorRoute(page: Page): Promise<Record<string, string>> {
  await addRole(page, "Lead");
  const keys: Record<string, string> = {};
  keys["Kickoff"] = await addNode(page, "milestone", "Kickoff");
  keys["Who leads"] = await addNode(page, "decision", "Who leads");
  await formField(page, "answer_type").getByLabel("Answer type").selectOption("entity");
  await pickByTitle(formField(page, "fills_role").getByLabel("Fills the role"), "Lead");
  await saveForm(page);
  keys["Include training"] = await addNode(page, "decision", "Include training");
  // A decision's form opens on its question and folds the rest (design 4.9).
  await expect(nodeForm(page).getByTestId("form-question")).toHaveAttribute("open", "");
  await expect(nodeForm(page).getByTestId("form-relevance")).not.toHaveAttribute("open");
  keys["Setup"] = await addNode(page, "group", "Setup");
  await openForm(page, "dates");
  await pickByTitle(formField(page, "opens_at").getByLabel("Opens at"), "Kickoff");
  await saveForm(page);
  keys["Training plan"] = await addNode(page, "deliverable", "Training plan", "Setup");
  await openForm(page, "relevance");
  await formField(page, "relevant_when").getByRole("button", { name: "Add a condition" }).click();
  await pickByTitle(formField(page, "relevant_when").getByLabel("Decision"), "Include training");
  await saveForm(page);
  keys["Handbook"] = await addNode(page, "deliverable", "Handbook", "Setup");
  await openForm(page, "dates");
  await formField(page, "due_by").getByRole("button", { name: "Add a rule" }).click();
  await pickByTitle(formField(page, "due_by").getByLabel("Also measure from"), "Kickoff");
  await formField(page, "due_by").getByRole("button", { name: /^Stop measuring from When the journey started/ }).click();
  await formField(page, "due_by").getByLabel("Offset in days").fill("10");
  await formField(page, "due_by").getByLabel("Before or after").selectOption("after");
  await saveForm(page);
  keys["Accounts"] = await addNode(page, "deliverable", "Accounts", "Setup");
  keys["Launch"] = await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  return keys;
}

test("A12, A1a: a route authored by hand from an empty draft, published, exported, and imported back to the same graph", async ({ page }) => {
  const route = await newRoute(page, "browser", routeName("Onboarding"));
  const keys = await authorRoute(page);
  expect(await offered(page)).toEqual(expect.arrayContaining(["final", "auto_reach"]));
  expect(await offered(page)).not.toEqual(expect.arrayContaining(["estimate"]));
  await openNode(page, keys["Who leads"] ?? "");
  expect(await offered(page)).toEqual(expect.arrayContaining(["prompt", "answer_type", "fills_role"]));
  expect(await offered(page)).not.toContain("choices");
  await expect(nodeCard(page, keys["Training plan"] ?? "")).toBeVisible();
  await open(page, "browser", `/routes/${route}/versions`);
  await page.getByTestId("route-actions").getByRole("button", { name: "Publish the draft" }).click();
  const version = page.locator('[data-testid="version"][data-version="1"]');
  await expect(version).toBeVisible();
  const exported = await savedText(page, () => version.getByRole("button", { name: "Export" }).click());
  expect(exported).toContain(`route: ${route}`);
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles({ name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(exported) });
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  const reimported = await savedText(page, () => page.getByTestId("route-actions").getByRole("button", { name: "Export the draft" }).click());
  expect(reimported).toBe(exported);
});

test("A3: a requirement drawn on the canvas lands; one on its own container is refused with the reason", async ({ page }) => {
  await newRoute(page, "browser", routeName("Edges"));
  const setup = await addNode(page, "group", "Setup");
  const plan = await addNode(page, "deliverable", "Plan", "Setup");
  const access = await addNode(page, "deliverable", "Access");
  await openNode(page, plan);
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  await expect(page.getByTestId("drawing-edge")).toHaveAttribute("data-from", plan);
  await nodeCard(page, setup).getByTestId("card-open").click();
  await expect(page.getByTestId("edge-refused")).toContainText("A3");
  await nodeCard(page, access).getByTestId("card-open").click();
  await expect(page.getByTestId("drawing-edge")).toHaveCount(0);
  await expect(structure(page).locator(`[data-testid="requirement"][data-node="${access}"]`)).toBeVisible();
  const edges = await section(page, "author-edges");
  await pickByTitle(edges.getByLabel("Require"), "Setup");
  await expect(edges.getByTestId("edge-refused")).toContainText("A3");
  await expect(edges.getByRole("button", { name: "Add the requirement" })).toBeDisabled();
});

test("A20, A15: a deliverable no chain links to the final milestone carries a notice, and an edge from it to the final report clears the notice without a reload", async ({ page }) => {
  await newRoute(page, "browser", routeName("Notices"));
  const report = await addNode(page, "deliverable", "Final report");
  await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  const edges = await section(page, "author-edges");
  await pickByTitle(edges.getByLabel("Require"), "Final report");
  await edges.getByRole("button", { name: "Add the requirement" }).click();
  const handbook = await addNode(page, "deliverable", "Handbook");
  await expect(nodeCard(page, handbook).getByTestId("card-notice")).toBeVisible();
  await expect(nodeCard(page, report).getByTestId("card-notice")).toHaveCount(0);
  await openNode(page, report);
  await pickByTitle((await section(page, "author-edges")).getByLabel("Require"), "Handbook");
  await (await section(page, "author-edges")).getByRole("button", { name: "Add the requirement" }).click();
  await expect(page.getByTestId("card-notice")).toHaveCount(0);
});

test("A15: a stage's id and both bounds, each wrong, are three violations at three fields", async ({ page }) => {
  await newRoute(page, "browser", routeName("Stages"));
  await addNode(page, "milestone", "Taken");
  const stage = await addNode(page, "group", "Stage");
  await addNode(page, "milestone", "Inside", "Stage");
  await openNode(page, stage);
  await openForm(page, "dates");
  await formField(page, "id").getByRole("textbox").fill("taken");
  await pickByTitle(formField(page, "opens_at").getByLabel("Opens at"), "Inside");
  await pickByTitle(formField(page, "closes_at").getByLabel("Closes at"), "Inside");
  await expect(nodeForm(page).getByTestId("preview")).toHaveAttribute("data-status", "rejected");
  for (const field of ["id", "opens_at", "closes_at"]) {
    await expect(formField(page, field)).toHaveAttribute("data-invalid", "true");
    await expect(formField(page, field).getByTestId("violation")).toHaveCount(1);
  }
  await page.getByTestId("node-form-save").click();
  await expect(formField(page, "id").getByTestId("violation")).toHaveCount(1);
  await expect(syncChip(page)).toHaveAttribute("data-state", "not-saved");
});

test("B4: on a fixture journey, a local node added, a route-copied title edited and reset to the route", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  const local = await addNode(page, "action", "Order the test data", "Setup");
  await expect((await section(page, "author-local-edits")).getByTestId("local-edits")).toHaveAttribute("data-status", "local");
  await expect(nodeCard(page, local)).toBeVisible();
  await openFromCanvas(page, "n_access");
  await formField(page, "title").getByRole("textbox").fill("Sandbox access");
  await saveForm(page);
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Sandbox access");
  await expect(formField(page, "title").getByTestId("edited-here")).toBeVisible();
  const edits = await section(page, "author-local-edits");
  const marker = edits.getByTestId("local-edit").filter({ hasText: "title" });
  await marker.getByRole("button", { name: "Reset to route" }).click();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Environment access");
  await expect(edits.getByTestId("local-edit")).toHaveCount(0);
  await expect(formField(page, "title").getByTestId("edited-here")).toHaveCount(0);
});

test("A18, B4: a decision a condition names removed with its cascade, then restored from its tombstone", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_comparison_set");
  await structure(page).getByTestId("remove-node").click();
  const dialog = page.getByTestId("cascade-dialog");
  await expect(dialog.locator('[data-testid="cascade-rewrite"][data-node="n_baseline"]')).toHaveAttribute("data-what", "condition");
  await expect(dialog.getByTestId("cascade-edge")).toContainText("Test plan");
  await expect(dialog.getByTestId("cascade-preview")).toHaveAttribute("data-status", "accepted");
  await dialog.getByRole("button", { name: "Confirm" }).click();
  await expect(nodeCard(page, "n_comparison_set")).toHaveCount(0);
  await openFromCanvas(page, "n_baseline");
  await openForm(page, "relevance");
  await expect(formField(page, "relevant_when").getByRole("button", { name: "Add a condition" })).toBeVisible();
  const stones = page.getByTestId("tombstones");
  await stones.locator("summary").click();
  const stone = stones.locator('[data-testid="tombstone"][data-node="n_comparison_set"]');
  await stone.getByRole("button", { name: "Restore as a local copy" }).click();
  await expect(stone.getByTestId("restored")).toBeVisible();
  await expect(page.getByTestId("node-card").filter({ hasText: "Comparison set" })).toHaveCount(1);
});

test("Identity and references: renaming a decision a condition names leaves the condition on it", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_comparison_set");
  await formField(page, "title").getByRole("textbox").fill("Benchmark set");
  await formField(page, "id").getByRole("textbox").fill("benchmark-set");
  await saveForm(page);
  await openFromCanvas(page, "n_baseline");
  await openForm(page, "relevance");
  await expect(nodeForm(page).getByTestId("condition-words")).toContainText("Benchmark set");
  await expect(formField(page, "relevant_when").getByLabel("Decision")).toHaveValue("n_comparison_set");
});

test("B10: a deliverable broken down by hand into two pieces", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_findings");
  const breakdown = await section(page, "author-breakdown");
  await breakdown.getByLabel("Piece title").fill("Draft the findings");
  await breakdown.getByRole("button", { name: "Another piece" }).click();
  await breakdown.getByLabel("Piece title").nth(1).fill("Review the findings");
  await breakdown.getByRole("button", { name: "Break it down" }).click();
  await expect(breakdown).toContainText("2 beneath it.");
  await expect(page.getByTestId("node-card").filter({ hasText: "Review the findings" })).toHaveCount(1);
});

test("A4: a choice added to a decision stays in its unsaved draft, and lands with Save", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_comparison_set");
  const choices = formField(page, "choices");
  await expect(choices.getByTestId("choice")).toHaveCount(3);
  await choices.getByLabel("New choice").fill("Industry report");
  await choices.getByRole("button", { name: "Add the choice" }).click();
  await expect(choices.getByTestId("choice")).toHaveCount(4);
  await expect(page.getByTestId("node-form-save")).toHaveText("Save (1 change)");
  await saveForm(page);
  await expect(choices.getByTestId("choice")).toHaveCount(4);
});

test("H5: a field changed elsewhere while a draft is open is not reverted by the draft's Save", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  const panel = await openFromCanvas(page, "n_access");
  await formField(page, "description").getByRole("textbox").fill("Ask the environment team.");
  await menuItem(panel, "rename");
  await page.getByTestId("rename").getByRole("textbox").fill("Sandbox access");
  await page.getByTestId("rename").getByRole("button", { name: "Save" }).click();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Sandbox access");
  await expect(formField(page, "title").getByRole("textbox")).toHaveValue("Sandbox access");
  await expect(page.getByTestId("node-form-save")).toHaveText("Save (1 change)");
  await saveForm(page);
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Sandbox access");
});
