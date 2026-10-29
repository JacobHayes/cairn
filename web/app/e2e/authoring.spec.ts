// Authoring on cairn demo: a small route
// authored by hand from an empty draft, with an edge drawn on the canvas and a refusal (A3), a notice cleared by a requirement (A20),
// published, exported, and imported back to the same graph (A12, A1a, A13); and, on a fixture
// journey, a local node added and a route-copied title edited and reset (B4). The forms'
// own rules (fields, violations, removal, conditions, breakdowns) are vitest's.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { importIntoDraft, openRoute, publishDraft, routeMenuItem, savedText, versionSelect } from "./around.ts";
import { addNode, addRole, formField, newRoute, nodeForm, openEditing, openForm, pickByTitle, routeName, saveForm, section, structure } from "./authoring.ts";
import { nodeCard, open, openFromCanvas, openJourney, readable, whenSettled } from "./shell.ts";

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
  await formField(page, "answer_type").getByLabel("Answer type").selectOption("identity");
  await pickByTitle(formField(page, "fills_role").getByLabel("Fills the role"), "Lead");
  await saveForm(page);
  keys["Include training"] = await addNode(page, "decision", "Include training");
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
  const route = await newRoute(page, routeName("Onboarding"));
  const keys = await authorRoute(page);
  await expect(nodeCard(page, keys["Training plan"] ?? "")).toBeVisible();
  // A requirement is drawn on the canvas by clicking cards, and one on its own container is refused with the engine's reason (A3).
  const [handbook, setup, accounts] = [keys["Handbook"] ?? "", keys["Setup"] ?? "", keys["Accounts"] ?? ""];
  await expect(nodeCard(page, handbook).getByTestId("card-notice")).toBeVisible();
  await openNode(page, handbook);
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  const banner = page.getByTestId("drawing-edge");
  await expect(banner).toHaveAttribute("data-from", handbook);
  await nodeCard(page, setup).getByTestId("card-open").click();
  await expect(page.locator(`[data-testid="violation"][data-code="edge_to_ancestor_or_descendant"]`)).toBeVisible();
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
  await openRoute(page, route);
  await publishDraft(page);
  const exported = await savedText(page, () => routeMenuItem(page, "menu-export"));
  expect(exported).toContain(`route: ${route}`);
  await importIntoDraft(page, { name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(exported) });
  await expect(versionSelect(page)).toContainText("Draft");
  const reimported = await savedText(page, () => routeMenuItem(page, "menu-export"));
  expect(reimported).toBe(exported);
});


test("on a fixture journey, a node added and a title edited", async ({ page }) => {
  await readable(page);
  await openEditing(page, "j_vendor_eval");
  const local = await addNode(page, "action", "Order the test data", "Setup");
  await expect(nodeCard(page, local)).toBeVisible();
  await openFromCanvas(page, "n_access");
  await formField(page, "title").getByRole("textbox").fill("Sandbox access");
  await saveForm(page);
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Sandbox access");
});

test("A3: Edit is one state entered from the journey menu, with the name editor beside the structure tools; Escape in a field belongs to the field and outside one stops drawing and keeps edit mode, on a phone too; leaving edit mode or opening another graph stops it too", async ({ page }) => {
  const banner = page.getByTestId("drawing-edge");
  const draw = async () => {
    await openFromCanvas(page, "n_access");
    await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
    await expect(banner).toHaveAttribute("data-from", "n_access");
  };
  await openJourney(page, "j_vendor_eval");
  await expect(page.getByTestId("authoring-bar")).toHaveCount(0);
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-edit").click();
  const bar = page.getByTestId("authoring-bar");
  await expect(bar).toBeVisible();
  await bar.getByRole("button", { name: "Edit the name, description and owners" }).click();
  await expect(bar.getByTestId("header-editor")).toBeVisible();
  await draw();
  // Escape in a field belongs to the field: drawing goes on. Outside one it stops the drawing and leaves the node open.
  const title = nodeForm(page).getByLabel("Title", { exact: true });
  await title.focus();
  await expect(title).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(banner).toBeVisible();
  await banner.getByText("Pick the node").click();
  // On a phone the drawing's banner is one banner above the graph.
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(banner).toHaveCount(1);
  await expect(banner).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(banner).toHaveCount(0);
  await expect(bar).toBeVisible();
  await page.setViewportSize({ width: 1280, height: 720 });
  await draw();
  await page.getByTestId("edit-done").click();
  await expect(page.getByTestId("authoring-bar")).toHaveCount(0);
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-edit").click();
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
  await expect(banner).toHaveCount(0);
  await draw();
  await open(page, "/journeys/j_launch?view=graph&edit=on");
  await expect(page.getByTestId("authoring-bar")).toBeVisible();
  await expect(banner).toHaveCount(0);
});

test("A1a, A15: Publish waits for every form holding input not yet sent, a blank offset is refused rather than read as 0, and a violation at a field is reachable from its control", async ({ page }) => {
  const route = await newRoute(page, routeName("Waits"));
  const first = await addNode(page, "milestone", "Kickoff");
  const report = await addNode(page, "deliverable", "Report");
  const publish = page.getByTestId("publish-draft");
  // A held Publish says why and points at what holds it: the form is drawn again, with the focus in it.
  const held = async (words: RegExp) => {
    await publish.click({ force: true });
    await expect(page.getByText(words).first()).toBeVisible();
  };
  // A palette typed in another route whose id only begins with this one's holds nothing here.
  await page.evaluate((id) => { sessionStorage.setItem(`cairn:draft:add-node:route:${id}-more`, JSON.stringify({ kind: "action", title: "Typed elsewhere", parent: "" })); }, route);
  await page.getByTestId("add-menu").click();
  await page.getByTestId("add-menu").click();
  await expect(publish).not.toHaveAttribute("data-held", /.+/);
  // The add-node palette's visible labels, and its typed title.
  await page.getByTestId("add-menu").click();
  const palette = page.getByTestId("add-node");
  for (const label of ["Kind", "Title", "Inside"]) {
    await expect(palette.locator(".label", { hasText: new RegExp(`^${label}$`, "i") })).toBeVisible();
  }
  await palette.getByLabel("New node title").fill("Another");
  // The press closes the palette; the held Publish opens it again at the title.
  await page.getByTestId("canvas").click({ position: { x: 5, y: 5 }, force: true });
  await expect(palette).toHaveCount(0);
  await held(/Save or discard your changes to the node being added first/);
  await expect(palette.getByLabel("New node title")).toBeFocused();
  await palette.getByLabel("New node title").fill("");
  // A resource form with input.
  await nodeCard(page, report).getByTestId("card-open").click();
  await (await section(page, "author-resources")).getByRole("button", { name: "Add a resource" }).click();
  await structure(page).getByTestId("resource-form").getByLabel("Resource title").fill("A tip");
  // A click that arrives with no pointer or key (a screen reader's) points at the form as well.
  await publish.evaluate((button) => { (button as HTMLElement).click(); });
  await expect(page.getByText(/Save or discard your changes to a resource of Report first/).first()).toBeVisible();
  await expect(structure(page).getByTestId("resource-form").getByLabel("Resource title")).toBeFocused();
  await structure(page).getByTestId("resource-form").getByRole("button", { name: "Cancel" }).click();
  // A role form with input.
  await page.getByTestId("roles-menu").click();
  const roles = page.getByTestId("roles-and-kinds").getByTestId("roles");
  await roles.getByRole("button", { name: "Add a role" }).click();
  await roles.getByTestId("role-form").getByLabel("role label").fill("Lead");
  await page.getByTestId("canvas").click({ position: { x: 5, y: 5 }, force: true });
  await held(/Save or discard your changes to a role first/);
  await expect(roles.getByTestId("role-form").getByLabel("role label")).toBeFocused();
  await roles.getByTestId("role-form").getByRole("button", { name: "Cancel" }).click();
  await expect(publish).not.toHaveAttribute("data-held", /.+/);
  // A blank offset is no number: the engine's message shows, and nothing is sent as 0.
  await nodeCard(page, report).getByTestId("card-open").click();
  await openForm(page, "dates");
  await formField(page, "due_by").getByRole("button", { name: "Add a rule" }).click();
  const offset = formField(page, "due_by").getByLabel("Offset in days");
  await offset.fill("");
  await expect(offset).toHaveValue("");
  await expect(formField(page, "due_by").getByTestId("problem")).toContainText("whole number");
  await expect(page.getByTestId("node-form-save")).toBeDisabled();
  await offset.fill("3");
  await expect(formField(page, "due_by").getByTestId("problem")).toHaveCount(0);
  // The engine's violation at a field is reachable from the field's control.
  await nodeCard(page, first).getByTestId("card-open").click();
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  await nodeCard(page, await addNode(page, "milestone", "Second")).getByTestId("card-open").click();
  await formField(page, "final").getByRole("checkbox").check();
  const violation = formField(page, "final").getByTestId("violation");
  await expect(violation).toBeVisible();
  const id = await whenSettled(page, () => violation.getAttribute("id"));
  expect(id).not.toBeNull();
  await expect(formField(page, "final").getByRole("checkbox")).toHaveAttribute("aria-describedby", new RegExp(`(^| )${id ?? ""}( |$)`));
});

test("A6: a role's id follows its label until the author types over it, and an unsent form is not lost to another role's Edit", async ({ page }) => {
  await newRoute(page, routeName("Roles"));
  await addRole(page, "Lead");
  const roles = page.getByTestId("roles-and-kinds").getByTestId("roles");
  await roles.getByRole("button", { name: "Add a role" }).click();
  const form = roles.getByTestId("role-form");
  await form.getByLabel("role label").fill("Sponsor");
  await expect(form.getByLabel("role id")).toHaveValue("sponsor");
  await form.getByLabel("role id").fill("chief");
  await form.getByLabel("role label").fill("Sponsor of the work");
  await expect(form.getByLabel("role id")).toHaveValue("chief");
  // Another role's Edit would replace the form: it waits, and the form keeps what was typed.
  const edit = roles.getByTestId("role").filter({ hasText: "Lead" }).getByRole("button", { name: "Edit" });
  await expect(edit).toHaveAttribute("aria-disabled", "true");
  await edit.click({ force: true });
  await expect(form.getByLabel("role label")).toHaveValue("Sponsor of the work");
});
