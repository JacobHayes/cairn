// The edit manager on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): a write belongs to the tab, not to the control that sent it. A rename
// whose answer comes after its screen was left stays on the chip across a reload until it is
// dealt with (H5, H6), and one whose answer was lost is resent with its patch id and lands once.
// A node's actions form that sent its write stays until that write ends: text typed since is
// never lost to the landing, closing it never drops the write, and a rejection that came after
// the screen was left waits under the form (D1, H5).
// The other write paths hold their refusals the same way: a screen's write, an authoring edit,
// and an import that replaces an open draft.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { expect, test, type Page } from "@playwright/test";

import { addNode, formField, newRoute, nodeForm, routeName, saveForm, structure } from "./authoring.ts";
import { fresh, goWithin, menuItem, nodeCard, nodePanel, open, openFromCanvas, openJourney, recentSaves, rename, renameOf, save, startRename, syncChip } from "./shell.ts";

const PATCHES = "**/api/journeys/*/patches";

/** Two pages on the hiring journey: `two` has started renaming `node`, then `one` renames it, so `two`'s save will conflict. */
async function conflicting(context: { newPage: () => Promise<Page> }, node: string) {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, node, mine);
  await rename(one, node, theirs);
  return { one, two, theirs, mine };
}

const problems = (page: Page) => page.getByTestId("sync-popover").getByTestId("sync-problem");

test("a rename rejected after its screen was left stays until discarded, across a reload", { tag: "@server" }, async ({ context }) => {
  const { two, mine } = await conflicting(context, "n_onsite");
  // The answer comes only after the screen is left: the request is held until then.
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await two.route(PATCHES, async (route) => {
    await held;
    await route.continue();
  });
  await save(two, "n_onsite");
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByTestId("node-detail")).toHaveCount(0);
  release();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  // Reloading keeps it, and Go to it brings the rename back with what was typed and what was refused.
  await two.reload();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await syncChip(two).click();
  await expect(problems(two)).toHaveAttribute("data-kind", "conflict");
  await problems(two).getByRole("button", { name: "Go to it" }).click();
  await expect(two.getByTestId("conflict")).toBeVisible();
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  // Discard clears it from the chip and closes the form.
  await syncChip(two).click();
  await problems(two).getByRole("button", { name: "Discard" }).click();
  await expect(problems(two)).toHaveCount(0);
  await expect(syncChip(two)).not.toHaveAttribute("data-state", /conflict|not-saved/);
  await expect(renameOf(two, "n_onsite")).toHaveCount(0);
});

test("Keep my edit keeps what was typed after the conflict, across a reload", { tag: "@server" }, async ({ context }) => {
  const { two } = await conflicting(context, "n_screen");
  await save(two, "n_screen");
  await expect(two.getByTestId("conflict")).toBeVisible();
  const later = fresh("Later");
  await renameOf(two, "n_screen").getByRole("textbox").fill(later);
  await two.reload();
  await expect(two.getByTestId("conflict")).toBeVisible();
  await expect(renameOf(two, "n_screen").getByRole("textbox")).toHaveValue(later);
  await two.getByRole("button", { name: "Keep my edit on the current version" }).click();
  await save(two, "n_screen");
  await expect(nodeCard(two, "n_screen").getByTestId("title")).toHaveText(later);
  await expect(renameOf(two, "n_screen")).toHaveCount(0);
});

test("a rename whose answer is lost is resent with its id and lands once", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_hiring");
  const ids: string[] = [];
  // The first request reaches the server and commits; its answer never reaches the page.
  await page.route(PATCHES, async (route) => {
    ids.push((route.request().postDataJSON() as { patch: { id: string } }).patch.id);
    if (ids.length === 1) {
      await route.fetch();
      await route.abort();
    } else {
      await route.continue();
    }
  });
  const title = fresh("Resent");
  await rename(page, "n_offer", title);
  // The commit is shown by the tick; the resend follows after its first wait.
  await expect.poll(() => ids.length).toBe(2);
  expect(ids[0]).toBe(ids[1]);
  await expect.poll(async () => (await recentSaves(page)).length).toBe(1);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

/** Holds the page's patch requests until the returned release; each request's mutations are recorded as it goes out. */
async function holding(page: Page) {
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const sent: unknown[][] = [];
  await page.route(PATCHES, async (route) => {
    sent.push((route.request().postDataJSON() as { patch: { mutations: unknown[] } }).patch.mutations);
    await held;
    await route.continue();
  });
  return { release, sent };
}

test("text typed after a form was sent is kept when the send lands, and was never sent, across a reload", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_launch");
  const { release, sent } = await holding(page);
  const panel = await openFromCanvas(page, "n_announcement");
  await menuItem(panel, "skip");
  const form = panel.getByTestId("skip-form");
  const first = fresh("Waiting on the vendor");
  await form.getByLabel("Why skip it").fill(first);
  await form.getByRole("button", { name: "Skip" }).click();
  await expect(form.getByText("Saving...")).toBeVisible();
  await expect(form.getByRole("button", { name: "Skip" })).toBeDisabled();
  const longer = `${first} until Friday`;
  await form.getByLabel("Why skip it").fill(longer);
  release();
  const kept = panel.getByTestId("kept");
  await expect(kept).toContainText("Skipped.");
  await expect(kept.getByTestId("kept-text")).toContainText(longer);
  await expect(panel.getByRole("button", { name: "Skip", exact: true })).toHaveCount(0);
  expect(JSON.stringify(sent)).toContain(first);
  expect(JSON.stringify(sent)).not.toContain("until Friday");
  await page.reload();
  await expect(kept).toContainText("Skipped.");
  await expect(kept.getByTestId("kept-text")).toContainText(longer);
  await kept.getByRole("button", { name: "Close" }).click();
  await expect(kept).toHaveCount(0);
  await expect(panel.getByTestId("detail-state")).toHaveText("skipped");
});

test("Done... sends the note it was given and keeps what was added meanwhile, and the node is done", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_launch");
  const { release, sent } = await holding(page);
  const panel = await openFromCanvas(page, "n_docs");
  await panel.getByRole("button", { name: "Done..." }).click();
  const evidence = panel.getByTestId("done-evidence");
  const link = `https://example.test/${fresh("docs").replace(" ", "-")}`;
  await evidence.getByLabel("Artifact address").fill(link);
  await evidence.getByRole("button", { name: "Link and mark done" }).click();
  await expect(evidence.getByText("Saving...")).toBeVisible();
  await evidence.getByLabel("Artifact address").fill(`${link}/later`);
  release();
  const kept = panel.getByTestId("kept");
  await expect(kept).toContainText("Marked done.");
  await expect(kept).toContainText(`${link}/later`);
  expect(JSON.stringify(sent)).toContain(link);
  expect(JSON.stringify(sent)).not.toContain("/later");
  await expect(panel.getByTestId("detail-state")).toHaveText("done");
  await page.reload();
  await expect(kept).toContainText(`${link}/later`);
});

test("closing a form that sent its write leaves the write going, and the node is skipped", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_launch");
  const { release } = await holding(page);
  const panel = await openFromCanvas(page, "n_retro");
  await menuItem(panel, "skip");
  const form = panel.getByTestId("skip-form");
  await form.getByLabel("Why skip it").fill(fresh("Not this time"));
  await form.getByRole("button", { name: "Skip" }).click();
  await form.getByRole("button", { name: "Close" }).click();
  await expect(form).toHaveCount(0);
  release();
  await expect(panel.getByTestId("detail-state")).toHaveText("skipped");
  await expect(nodeCard(page, "n_retro")).toBeVisible();
});

test("a node action rejected after its screen was left waits under its form, across a reload", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_launch");
  await openJourney(two, "server", "j_launch");
  // Page two has the form open; page one skips the same node meanwhile, so page two's skip is stale.
  const panelTwo = await openFromCanvas(two, "n_launch");
  await menuItem(panelTwo, "skip");
  const mine = fresh("Mine");
  await panelTwo.getByTestId("skip-form").getByLabel("Why skip it").fill(mine);
  const panelOne = await openFromCanvas(one, "n_launch");
  await menuItem(panelOne, "skip");
  await panelOne.getByTestId("skip-form").getByLabel("Why skip it").fill(fresh("Theirs"));
  await panelOne.getByTestId("skip-form").getByRole("button", { name: "Skip" }).click();
  await expect(panelOne.getByTestId("detail-state")).toHaveText("skipped");
  const { release } = await holding(two);
  await panelTwo.getByTestId("skip-form").getByRole("button", { name: "Skip" }).click();
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByTestId("node-detail")).toHaveCount(0);
  release();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^(CONFLICT|NOT SAVED) · 1\./);
  await two.reload();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^(CONFLICT|NOT SAVED) · 1\./);
  await syncChip(two).click();
  await two.getByTestId("sync-popover").getByTestId("sync-problem").getByRole("button", { name: "Go to it" }).click();
  // The form is back with what was typed, and the refusal under it.
  const restored = nodePanel(two, "n_launch");
  await expect(restored.getByTestId("skip-form").getByLabel("Why skip it")).toHaveValue(mine);
  await expect(restored.locator('[data-testid="conflict"], [data-testid="invalid"]')).toBeVisible();
});

test("a new route refused for an id in use stays under its control and on the chip, across a reload, until discarded", async ({ page }) => {
  await open(page, "browser", "/routes/new");
  const form = page.getByTestId("new-route");
  // The name's slug is the id of a route the fixtures already hold.
  await form.getByLabel("New route name").fill("Hiring loop");
  await form.getByTestId("new-create").click();
  await expect(form.getByTestId("refused")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  // Away from the control it is still counted, and a reload keeps it under the control.
  await goWithin(page, "/");
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await goWithin(page, "/routes/new");
  await expect(form.getByTestId("refused")).toBeVisible();
  await page.reload();
  await expect(form.getByTestId("refused")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  // Discard on the chip clears it here too, and it does not come back.
  await syncChip(page).click();
  await problems(page).getByRole("button", { name: "Discard" }).click();
  await expect(form.getByTestId("refused")).toHaveCount(0);
  await page.reload();
  await expect(form.getByTestId("refused")).toHaveCount(0);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("an import refused for an open draft keeps its file across a reload, and imports over the draft", async ({ page }) => {
  const buffer = readFileSync(fileURLToPath(new URL("../../../fixtures/hiring-loop/route.yaml", import.meta.url)));
  const file = { name: "hiring-loop.yaml", mimeType: "text/yaml", buffer };
  await open(page, "browser", "/routes/hiring-loop/versions");
  const actions = page.getByTestId("route-actions");
  const importing = actions.getByTestId("import");
  const choose = () => actions.getByLabel("Import a file as a new draft").setInputFiles(file);
  await choose();
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  // Another draft is open now: the file is refused, and the option is to replace it.
  await choose();
  await expect(importing.locator('[data-testid="violation"][data-code="draft_exists"]')).toBeVisible();
  await importing.getByRole("button", { name: "Discard the open draft and import" }).click();
  await expect(importing.getByTestId("refused")).toHaveCount(0);
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  // Refused again, the file is kept with the refusal across a reload, which also drops the draft the page held.
  await choose();
  await expect(importing.getByTestId("refused")).toBeVisible();
  await page.reload();
  await expect(importing.getByTestId("refused")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await importing.getByRole("button", { name: "Discard the open draft and import" }).click();
  await expect(importing.getByTestId("refused")).toHaveCount(0);
  await expect(page.getByTestId("draft")).toHaveAttribute("data-status", "open");
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("an authoring edit the engine refuses stays at its field across a reload, until discarded", { tag: "@server" }, async ({ page }) => {
  await newRoute(page, "server", routeName("Refusal"));
  const first = await addNode(page, "milestone", "Kickoff");
  await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  // A second final milestone is one too many: the engine's preview says so, and Save sends it anyway.
  await nodeCard(page, first).getByTestId("card-open").click();
  await expect(structure(page)).toHaveAttribute("data-node", first);
  await formField(page, "final").getByRole("checkbox").check();
  await nodeForm(page).getByTestId("node-form-save").click();
  const refused = formField(page, "final").locator('[data-testid="violation"][data-code="several_final_milestones"]');
  await expect(refused).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await page.reload();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await expect(structure(page)).toHaveAttribute("data-node", first);
  await expect(refused).toBeVisible();
  // Discard on the chip drops the refused edit, and the form with it.
  await syncChip(page).click();
  await problems(page).getByRole("button", { name: "Discard" }).click();
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});
