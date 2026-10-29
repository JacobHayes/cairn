// The edit manager on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): a write belongs to the tab, not to the control that sent it. A rename
// whose answer comes after its screen was left stays on the chip until it is
// dealt with (H5, H6), and one whose answer was lost is resent with its patch id and lands once.
// A node's actions form that sent its write stays until that write ends: text typed since is
// never lost to the landing, closing it never drops the write, and a rejection that came after
// the screen was left waits under the form (D1, H5).
// The other write paths hold their refusals the same way: a screen's write, an authoring edit,
// and an import that replaces an open draft.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import type { BrowserContext, Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { importIntoDraft, openRoute, versionSelect } from "./around.ts";
import { addNode, formField, newRoute, nodeForm, routeName, saveForm, structure } from "./authoring.ts";
import { fresh, goWithin, menuItem, nodeCard, nodePanel, open, openFromCanvas, openJourney, ownJourney, readable, recentSaves, rename, renameOf, save, startRename, syncChip } from "./shell.ts";

const PATCHES = "**/api/journeys/*/patches";

/** Two pages on a journey of its own started from the hiring loop: `two` has started renaming `node`, then `one` renames it, so `two`'s save will conflict. */
async function conflicting(context: BrowserContext, node: string) {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await Promise.all([readable(one), readable(two)]);
  const { id } = await ownJourney(context.request, "hiring-loop");
  await openJourney(one, "server", id);
  await openJourney(two, "server", id);
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, node, mine);
  await rename(one, node, theirs);
  return { one, two, theirs, mine };
}

const problems = (page: Page) => page.getByTestId("sync-popover").getByTestId("sync-problem");

test("a rename rejected after its screen was left stays until discarded", { tag: "@server" }, async ({ context }) => {
  const { two, mine } = await conflicting(context, "n_onsite");
  // The rename form's own limit stops a title that is too long, with the reason, before anything is sent.
  await renameOf(two, "n_onsite").getByRole("textbox").fill("x".repeat(300));
  await expect(renameOf(two, "n_onsite").getByTestId("problem")).toBeVisible();
  await expect(renameOf(two, "n_onsite").getByRole("button", { name: "Save" })).toBeDisabled();
  await renameOf(two, "n_onsite").getByRole("textbox").fill(mine);
  await expect(renameOf(two, "n_onsite").getByTestId("problem")).toHaveCount(0);
  await expect(renameOf(two, "n_onsite").getByRole("button", { name: "Save" })).toBeEnabled();
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
  // Go to it brings the rename back with what was typed and what was refused.
  await syncChip(two).click();
  await expect(problems(two)).toHaveAttribute("data-kind", "conflict");
  await problems(two).getByRole("button", { name: "Go to it" }).click();
  await expect(two.locator('[data-testid="refused"][data-rejection="stale"]')).toBeVisible();
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  // Discard clears it from the chip and closes the form.
  await syncChip(two).click();
  await problems(two).getByRole("button", { name: "Discard" }).click();
  await expect(problems(two)).toHaveCount(0);
  await expect(syncChip(two)).not.toHaveAttribute("data-state", /conflict|not-saved/);
  await expect(renameOf(two, "n_onsite")).toHaveCount(0);
});

test("a rename whose answer is lost is resent with its id and lands once", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", (await ownJourney(page.request, "hiring-loop")).id);
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

/**
 * Holds the page's patch requests until the next `release()`, which lets the held ones go and
 * holds the ones after it again; each request's mutations are recorded as it goes out.
 */
async function holding(page: Page) {
  const gate = () => {
    let open: () => void = () => undefined;
    const closed = new Promise<void>((resolve) => {
      open = resolve;
    });
    return { open, closed };
  };
  let current = gate();
  const sent: unknown[][] = [];
  await page.route(PATCHES, async (route) => {
    sent.push((route.request().postDataJSON() as { patch: { mutations: unknown[] } }).patch.mutations);
    await current.closed;
    await route.continue();
  });
  return {
    release: () => {
      const { open } = current;
      current = gate();
      open();
    },
    sent,
  };
}

test("a form that sent its write keeps text typed since when the send lands, and closing it leaves the write going, across a reload", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", (await ownJourney(page.request, "product-launch")).id);
  const { release, sent } = await holding(page);
  // Text typed after the send is kept when it lands, and was never sent.
  const skipping = await openFromCanvas(page, "n_announcement");
  await menuItem(skipping, "skip");
  const form = skipping.getByTestId("skip-form");
  const first = fresh("Waiting on the vendor");
  await form.getByLabel("Why skip it").fill(first);
  await form.getByRole("button", { name: "Skip" }).click();
  await expect(form.getByText("Saving...")).toBeVisible();
  await expect(form.getByRole("button", { name: "Skip" })).toBeDisabled();
  const longer = `${first} until Friday`;
  await form.getByLabel("Why skip it").fill(longer);
  release();
  const kept = skipping.getByTestId("kept");
  await expect(kept).toContainText("Skipped.");
  await expect(kept.getByTestId("kept-text")).toContainText(longer);
  await expect(skipping.getByRole("button", { name: "Skip", exact: true })).toHaveCount(0);
  expect(JSON.stringify(sent)).toContain(first);
  expect(JSON.stringify(sent)).not.toContain("until Friday");
  await page.reload();
  await expect(kept).toContainText("Skipped.");
  await expect(kept.getByTestId("kept-text")).toContainText(longer);
  await kept.getByRole("button", { name: "Close" }).click();
  await expect(kept).toHaveCount(0);
  await expect(skipping.getByTestId("detail-state")).toHaveText("skipped");
  // Done... sends the note it was given and keeps what was added meanwhile.
  sent.length = 0;
  const done = await openFromCanvas(page, "n_docs");
  await done.getByRole("button", { name: "Mark done…" }).click();
  const evidence = done.getByTestId("done-evidence");
  const link = `https://example.test/${fresh("docs").replace(" ", "-")}`;
  await evidence.getByLabel("Artifact link").fill(link);
  await evidence.getByRole("button", { name: "Add link and mark done" }).click();
  await expect(evidence.getByText("Saving...")).toBeVisible();
  await evidence.getByLabel("Artifact link").fill(`${link}/later`);
  release();
  const keptDone = done.getByTestId("kept");
  await expect(keptDone).toContainText("Marked done.");
  await expect(keptDone).toContainText(`${link}/later`);
  expect(JSON.stringify(sent)).toContain(link);
  expect(JSON.stringify(sent)).not.toContain("/later");
  await expect(done.getByTestId("detail-state")).toHaveText("done");
  await page.reload();
  await expect(keptDone).toContainText(`${link}/later`);
  // Closing a form whose write is out leaves the write going.
  const closing = await openFromCanvas(page, "n_retro");
  await menuItem(closing, "skip");
  const closed = closing.getByTestId("skip-form");
  await closed.getByLabel("Why skip it").fill(fresh("Not this time"));
  await closed.getByRole("button", { name: "Skip" }).click();
  await closed.getByRole("button", { name: "Close" }).click();
  await expect(closed).toHaveCount(0);
  release();
  await expect(closing.getByTestId("detail-state")).toHaveText("skipped");
  await expect(nodeCard(page, "n_retro")).toBeVisible();
});

test("a node action rejected after its screen was left waits under its form", { tag: "@server" }, async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await Promise.all([readable(one), readable(two)]);
  const { id } = await ownJourney(context.request, "product-launch");
  await openJourney(one, "server", id);
  await openJourney(two, "server", id);
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
  await syncChip(two).click();
  await two.getByTestId("sync-popover").getByTestId("sync-problem").getByRole("button", { name: "Go to it" }).click();
  // The form is back with what was typed, and the refusal under it.
  const restored = nodePanel(two, "n_launch");
  await expect(restored.getByTestId("skip-form").getByLabel("Why skip it")).toHaveValue(mine);
  await expect(restored.getByTestId("refused")).toBeVisible();
});

async function refusedNewRoute(page: Page) {
  await open(page, "browser", "/routes/new");
  const form = page.getByTestId("new-route");
  // The name's slug is the id of a route the fixtures already hold.
  await form.getByLabel("New route name").fill("Hiring loop");
  await form.getByTestId("new-create").click();
  await expect(form.getByTestId("refused")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  // Away from the control it is still counted, and back at it the refusal is under the control.
  await goWithin(page, "/");
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await goWithin(page, "/routes/new");
  await expect(form.getByTestId("refused")).toBeVisible();
  // Discard on the chip clears it here too.
  await syncChip(page).click();
  await problems(page).getByRole("button", { name: "Discard" }).click();
  await expect(form.getByTestId("refused")).toHaveCount(0);
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
}

async function refusedImport(page: Page) {
  const buffer = readFileSync(fileURLToPath(new URL("../../../fixtures/hiring-loop/route.yaml", import.meta.url)));
  const file = { name: "hiring-loop.yaml", mimeType: "text/yaml", buffer };
  await openRoute(page, "browser", "hiring-loop");
  const importing = page.getByTestId("route-menu-menu").getByTestId("import");
  const choose = () => importIntoDraft(page, file);
  await choose();
  await expect(versionSelect(page)).toContainText("Draft");
  // Another draft is open now: the file is refused, and the option is to replace it.
  await choose();
  await expect(importing.locator('[data-testid="violation"][data-code="draft_exists"]')).toBeVisible();
  await importing.getByRole("button", { name: "Discard the open draft and import" }).click();
  await expect(importing.getByTestId("refused")).toHaveCount(0);
  await expect(versionSelect(page)).toContainText("Draft");
  // Refused again, the file is kept with the refusal.
  await choose();
  await expect(importing.getByTestId("refused")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await importing.getByRole("button", { name: "Discard the open draft and import" }).click();
  await expect(importing.getByTestId("refused")).toHaveCount(0);
  await expect(versionSelect(page)).toContainText("Draft");
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
}

test("a new route refused for an id in use, and an import refused for an open draft, stay under their control and on the chip until discarded", async ({ page }) => {
  await refusedNewRoute(page);
  await refusedImport(page);
});

test("an authoring edit the engine refuses stays at its field until discarded", { tag: "@server" }, async ({ page }) => {
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
  await expect(structure(page)).toHaveAttribute("data-node", first);
  await expect(refused).toBeVisible();
  // Discard on the chip drops the refused edit, and the form with it.
  await syncChip(page).click();
  await problems(page).getByRole("button", { name: "Discard" }).click();
  await expect(syncChip(page)).not.toHaveAttribute("data-state", "not-saved");
});

test("an Enter pressed before the engine has read the typed title saves it as soon as it has", async ({ page }) => {
  await openJourney(page, "browser", "j_launch");
  const title = fresh("Quick");
  await startRename(page, "n_docs", title);
  await renameOf(page, "n_docs").getByRole("textbox").press("Enter");
  await expect(nodeCard(page, "n_docs").getByTestId("title")).toHaveText(title);
});
