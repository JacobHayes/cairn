// Brief 5.6 on the in-browser host, seeded from the fixtures on each load: a small route
// authored by hand from an empty draft, with an edge drawn on the canvas and a refusal (A3), a notice cleared by a requirement (A20),
// published, exported, and imported back to the same graph (A12, A1a, A13); and, on a fixture
// journey, a local node added and a route-copied title edited and reset (B4). The forms'
// own rules (fields, violations, removal, conditions, breakdowns) are vitest's.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { importIntoDraft, openRoute, publishDraft, routeMenuItem, savedText, versionSelect } from "./around.ts";
import { addNode, addRole, formField, newRoute, nodeForm, openEditing, openForm, pickByTitle, routeName, saveForm, section, structure } from "./authoring.ts";
import { nodeCard, open, openFromCanvas, openJourney } from "./shell.ts";

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
  // A requirement is drawn on the canvas by clicking cards, and one on its own container is refused with the reason (A3).
  const [handbook, setup, accounts] = [keys["Handbook"] ?? "", keys["Setup"] ?? "", keys["Accounts"] ?? ""];
  await expect(nodeCard(page, handbook).getByTestId("card-notice")).toBeVisible();
  await openNode(page, handbook);
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  const banner = page.getByTestId("drawing-edge");
  await expect(banner).toHaveAttribute("data-from", handbook);
  // Escape in a field belongs to the field: drawing goes on. Outside one it stops the drawing and leaves the node open.
  const title = nodeForm(page).getByLabel("Title", { exact: true });
  await title.focus();
  await expect(title).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(banner).toBeVisible();
  await banner.getByText("Pick the node").click();
  await page.keyboard.press("Escape");
  await expect(banner).toHaveCount(0);
  await expect(structure(page)).toHaveAttribute("data-node", handbook);
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  await expect(banner).toHaveAttribute("data-from", handbook);
  await nodeCard(page, setup).getByTestId("card-open").click();
  await expect(page.getByTestId("edge-refused")).toBeVisible();
  await nodeCard(page, accounts).getByTestId("card-open").click();
  await expect(page.getByTestId("drawing-edge")).toHaveCount(0);
  await expect(structure(page).locator(`[data-testid="requirement"][data-node="${accounts}"]`)).toBeVisible();
  // A deliverable no chain links to the final milestone carries a notice; a requirement from the final milestone clears it, and what it requires, without a reload (A20).
  await openNode(page, keys["Launch"] ?? "");
  const edges = await section(page, "author-edges");
  await pickByTitle(edges.getByLabel("Require"), "Handbook");
  await edges.getByRole("button", { name: "Add the requirement" }).click();
  await expect(nodeCard(page, handbook).getByTestId("card-notice")).toHaveCount(0);
  await expect(nodeCard(page, accounts).getByTestId("card-notice")).toHaveCount(0);
  await openRoute(page, "browser", route);
  await publishDraft(page);
  const exported = await savedText(page, () => routeMenuItem(page, "menu-export"));
  expect(exported).toContain(`route: ${route}`);
  await importIntoDraft(page, { name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(exported) });
  await expect(versionSelect(page)).toContainText("Draft");
  const reimported = await savedText(page, () => routeMenuItem(page, "menu-export"));
  expect(reimported).toBe(exported);
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


test("Edit is one state entered from the journey menu: structure tools and the name editor together, left with Done", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("authoring-bar")).toHaveCount(0);
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-edit").click();
  const bar = page.getByTestId("authoring-bar");
  await expect(bar).toBeVisible();
  await bar.getByRole("button", { name: "Edit the name and description" }).click();
  await expect(bar.getByTestId("header-editor")).toBeVisible();
  await page.getByTestId("edit-done").click();
  await expect(bar).toHaveCount(0);
});


test("A3: on a journey's graph in edit mode, Escape stops drawing and keeps edit mode; leaving edit mode or opening another graph stops it too", async ({ page }) => {
  const banner = page.getByTestId("drawing-edge");
  const draw = async () => {
    await openFromCanvas(page, "n_access");
    await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
    await expect(banner).toHaveAttribute("data-from", "n_access");
  };
  await openEditing(page, "browser", "j_vendor_eval");
  await draw();
  await page.keyboard.press("Escape");
  await expect(banner).toHaveCount(0);
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
  await draw();
  await page.getByTestId("edit-done").click();
  await expect(page.getByTestId("authoring-bar")).toHaveCount(0);
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-edit").click();
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
  await expect(banner).toHaveCount(0);
  await draw();
  await open(page, "browser", "/journeys/j_launch?view=graph&edit=on");
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
  await expect(banner).toHaveCount(0);
});
