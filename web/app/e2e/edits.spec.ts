// The edit manager on the server host, against `cairn demo` (the `server` project of
// playwright.config.ts): a write belongs to the tab, not to the control that sent it. A rename
// whose answer comes after its screen was left stays on the chip across a reload until it is
// dealt with (H5, H6), and one whose answer was lost is resent with its patch id and lands once.
import { expect, test, type Page } from "@playwright/test";

import { fresh, nodeCard, openJourney, recentSaves, rename, renameOf, save, startRename, syncChip } from "./shell.ts";

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
