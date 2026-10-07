// The proof's pictures for brief 5.6 (briefs/proof/5.6/prove.sh): a screenshot of each
// acceptance state of authoring, a route by hand from an empty draft and a fixture journey's
// structure, a short video of the main flow, and the values the README tabulates, written to
// CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show, so a picture of the
// wrong state fails the run. Everything runs on the in-browser host, seeded on each load.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

import { savedText } from "../e2e/around.ts";
import { addNode, addRole, dismissNotices, formField, newRoute, nodeForm, openEditing, pickByTitle, saveForm, section, structure } from "../e2e/authoring.ts";
import { nodeCard, open, openFromCanvas } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`), fullPage: true });
const beat = (page: Page) => page.waitForTimeout(700);
const values: Record<string, unknown> = {};
const record = (name: string, value: unknown) => {
  values[name] = value;
  writeFileSync(join(out, "authoring.json"), JSON.stringify(values, null, 2));
};

test.use({ viewport: { width: 1280, height: 900 } });
test.describe.configure({ mode: "serial" });

async function openNode(page: Page, key: string): Promise<void> {
  await nodeCard(page, key).getByTestId("card-open").click();
  await expect(structure(page)).toHaveAttribute("data-node", key);
}

/** The fields a form offers, by name. */
function offered(page: Page): Promise<string[]> {
  return nodeForm(page).getByTestId("author-field").evaluateAll((fields) => fields.map((field) => field.getAttribute("data-field") ?? ""));
}

async function conditionOn(field: Locator, decision: string): Promise<void> {
  await field.getByRole("button", { name: "Add a condition" }).click();
  await pickByTitle(field.getByLabel("Decision"), decision);
}

test("A12: a route authored by hand from an empty draft, published and exported", async ({ page }) => {
  const route = await newRoute(page, "browser", "Team onboarding");
  await shot(page, "2-empty-draft");
  await addRole(page, "Lead");
  await addNode(page, "milestone", "Kickoff");
  const kinds: Record<string, string[]> = { milestone: await offered(page) };
  const who = await addNode(page, "decision", "Who leads");
  await formField(page, "answer_type").getByLabel("Answer type").selectOption("entity");
  await pickByTitle(formField(page, "fills_role").getByLabel("Fills the role"), "Lead");
  await saveForm(page);
  kinds["decision (a person)"] = await offered(page);
  await shot(page, "3-a-decision-fills-a-role");
  await addNode(page, "decision", "Include training");
  const setup = await addNode(page, "group", "Setup");
  await pickByTitle(formField(page, "opens_at").getByLabel("Opens at"), "Kickoff");
  await saveForm(page);
  kinds["group"] = await offered(page);
  await addNode(page, "deliverable", "Training plan", "Setup");
  kinds["deliverable"] = await offered(page);
  await conditionOn(formField(page, "relevant_when"), "Include training");
  await saveForm(page);
  await expect(nodeForm(page).getByTestId("condition-words")).toContainText("Include training");
  await shot(page, "4-a-condition");
  await addNode(page, "deliverable", "Handbook", "Setup");
  const rule = formField(page, "due_by");
  await rule.getByRole("button", { name: "Add a rule" }).click();
  await pickByTitle(rule.getByLabel("Also measure from"), "Kickoff");
  await rule.getByRole("button", { name: /^Stop measuring from When the journey started/ }).click();
  await rule.getByLabel("Offset in days").fill("10");
  await rule.getByLabel("Before or after").selectOption("after");
  await saveForm(page);
  await shot(page, "5-a-date-rule");
  const accounts = await addNode(page, "deliverable", "Accounts", "Setup");
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  await nodeCard(page, setup).getByTestId("card-open").click();
  await expect(page.getByTestId("edge-refused")).toBeVisible();
  await shot(page, "6-an-edge-to-its-own-stage-refused");
  await nodeCard(page, who).getByTestId("card-open").click();
  await expect(structure(page).locator(`[data-testid="requirement"][data-node="${who}"]`)).toBeVisible();
  await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  await page.getByTestId("roles-and-kinds").locator("summary").first().click();
  await openNode(page, accounts);
  await shot(page, "7-the-route-on-its-canvas");
  record("kinds", kinds);
  await open(page, "browser", `/routes/${route}/versions`);
  await page.getByTestId("route-actions").getByRole("button", { name: "Publish the draft" }).click();
  const version = page.locator('[data-testid="version"][data-version="1"]');
  await expect(version).toBeVisible();
  const exported = await savedText(page, () => version.getByRole("button", { name: "Export" }).click());
  await page.getByTestId("route-actions").getByLabel("Import a file as a new draft").setInputFiles({ name: `${route}.yaml`, mimeType: "text/yaml", buffer: Buffer.from(exported) });
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  const reimported = await savedText(page, () => page.getByTestId("route-actions").getByRole("button", { name: "Export the draft" }).click());
  expect(reimported).toBe(exported);
  record("export", { route, bytes: exported.length, identical: reimported === exported, file: exported });
  await shot(page, "8-published-exported-and-imported-back");
});

test("A15: three violations at three fields", async ({ page }) => {
  await newRoute(page, "browser", "Stage checks");
  await addNode(page, "milestone", "Taken");
  const stage = await addNode(page, "group", "Stage");
  await addNode(page, "milestone", "Inside", "Stage");
  await openNode(page, stage);
  await formField(page, "id").getByRole("textbox").fill("taken");
  await pickByTitle(formField(page, "opens_at").getByLabel("Opens at"), "Inside");
  await pickByTitle(formField(page, "closes_at").getByLabel("Closes at"), "Inside");
  await expect(nodeForm(page).getByTestId("preview")).toHaveAttribute("data-status", "rejected");
  const fields = await nodeForm(page).locator('[data-testid="author-field"][data-invalid="true"]').evaluateAll((found) => found.map((field) => field.getAttribute("data-field")));
  expect(fields).toEqual(["id", "opens_at", "closes_at"]);
  const codes = await nodeForm(page).getByTestId("violation").evaluateAll((found) => found.map((violation) => violation.getAttribute("data-code")));
  record("violations", { fields, codes });
  await shot(page, "9-three-violations-at-three-fields");
});

test("B4, A18: a fixture journey's structure: a local node, a title reset to the route, a removal's cascade, a tombstone restored", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  const local = await addNode(page, "action", "Order the test data", "Setup");
  await shot(page, "10-journey-edit-mode-a-local-node");
  await openFromCanvas(page, "n_access");
  await formField(page, "title").getByRole("textbox").fill("Sandbox access");
  await saveForm(page);
  const edits = await section(page, "author-local-edits");
  await expect(edits.getByTestId("local-edit")).toHaveCount(1);
  await shot(page, "11-a-route-copied-title-edited-here");
  await edits.getByRole("button", { name: "Reset to route" }).click();
  await expect(nodeCard(page, "n_access").getByTestId("title")).toHaveText("Environment access");
  await dismissNotices(page);
  await openFromCanvas(page, "n_comparison_set");
  await structure(page).getByTestId("remove-node").click();
  const dialog = page.getByTestId("cascade-dialog");
  await expect(dialog.getByTestId("cascade-preview")).toHaveAttribute("data-status", "accepted");
  const rewrites = await dialog.getByTestId("cascade-rewrite").evaluateAll((found) => found.map((each) => [each.getAttribute("data-node"), each.getAttribute("data-what")]));
  const removed = await dialog.getByTestId("cascade-edge").allTextContents();
  await dialog.scrollIntoViewIfNeeded();
  await shot(page, "12-a-removal-and-its-cascade");
  await dialog.getByRole("button", { name: "Confirm" }).click();
  await expect(nodeCard(page, "n_comparison_set")).toHaveCount(0);
  await dismissNotices(page);
  const stones = page.getByTestId("tombstones");
  await stones.locator("summary").click();
  await stones.locator('[data-testid="tombstone"][data-node="n_comparison_set"]').getByRole("button", { name: "Restore as a local copy" }).click();
  await expect(stones.getByTestId("restored")).toBeVisible();
  await dismissNotices(page);
  await shot(page, "14-restored-as-a-local-copy");
  record("journey", { local, rewrites, removed });
});

test("B10: a deliverable broken down by hand", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_findings");
  const breakdown = await section(page, "author-breakdown");
  await breakdown.getByLabel("Piece title").fill("Draft the findings");
  await breakdown.getByRole("button", { name: "Another piece" }).click();
  await breakdown.getByLabel("Piece title").nth(1).fill("Review the findings");
  await breakdown.getByRole("button", { name: "Break it down" }).click();
  await expect(breakdown).toContainText("2 beneath it.");
  await dismissNotices(page);
  await shot(page, "15-broken-down-by-hand");
});

test("the dark theme and a narrow screen", async ({ browser, baseURL }) => {
  const dark = await browser.newPage({ baseURL: baseURL ?? "", colorScheme: "dark", viewport: { width: 1280, height: 900 } });
  await openEditing(dark, "browser", "j_vendor_eval");
  await openFromCanvas(dark, "n_baseline");
  await shot(dark, "16-dark-theme");
  await dark.close();
  const narrow = await browser.newPage({ baseURL: baseURL ?? "", viewport: { width: 390, height: 844 } });
  await openEditing(narrow, "browser", "j_vendor_eval");
  await openFromCanvas(narrow, "n_plan");
  expect(await narrow.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await shot(narrow, "17-narrow-screen");
  await narrow.close();
});

test("the main flow, on video", async ({ browser, baseURL }) => {
  const context = await browser.newContext({ baseURL: baseURL ?? "", viewport: { width: 1200, height: 760 }, recordVideo: { dir: join(out, "video"), size: { width: 1200, height: 760 } } });
  const page = await context.newPage();
  await newRoute(page, "browser", "Supplier review");
  await beat(page);
  await addNode(page, "decision", "Review on site");
  await beat(page);
  await addNode(page, "group", "Visit");
  await beat(page);
  await addNode(page, "deliverable", "Travel plan", "Visit");
  await conditionOn(formField(page, "relevant_when"), "Review on site");
  await beat(page);
  await saveForm(page);
  await beat(page);
  await addNode(page, "milestone", "Decision");
  await beat(page);
  await page.getByRole("link", { name: "Versions and journeys" }).click();
  await page.getByTestId("route-actions").getByRole("button", { name: "Publish the draft" }).click();
  await expect(page.locator('[data-testid="version"][data-version="1"]')).toBeVisible();
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
});
