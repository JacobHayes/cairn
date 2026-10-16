// The proof's media for kept edits (briefs/proof/2026-10-16-edits-are-kept/prove.sh): a
// rename the server rejects, still on the chip after leaving its screen and after a reload,
// brought back with Go to it and cleared with Discard (one flow, a screenshot of each state and
// a short video of it), a rename whose answer never comes, resent until it asks for Retry, and
// an authoring edit the engine refuses, still at its field after a reload.
// Written to CAIRN_PROOF_OUT. Steps only wait for the state they picture; the e2e specs assert
// it. Runs on the server host, over the fixtures the binary starts with.
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { addNode, formField, newRoute, nodeForm, routeName, saveForm, structure } from "../e2e/authoring.ts";
import { fresh, nodeCard, openJourney, rename, renameOf, save, startRename, syncChip } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(700);
const problems = (page: Page) => page.getByTestId("sync-popover").getByTestId("sync-problem");

test("a rejected rename stays until it is dealt with, on video", { tag: "@server" }, async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    baseURL: baseURL ?? "",
    viewport: { width: 1100, height: 720 },
    recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 720 } },
  });
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "server", "j_hiring");
  await openJourney(two, "server", "j_hiring");
  // Someone else renames the node while this page is still typing its own title.
  const [theirs, mine] = [fresh("Theirs"), fresh("Mine")];
  await startRename(two, "n_onsite", mine);
  await rename(one, "n_onsite", theirs);
  await save(two, "n_onsite");
  await expect(two.getByTestId("conflict")).toBeVisible();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await beat(two);
  await shot(two, "1-rejected");
  // Away from the rename, it is still on the chip.
  await two.getByRole("link", { name: "Journeys" }).click();
  await expect(two.getByTestId("node-detail")).toHaveCount(0);
  await syncChip(two).click();
  await expect(problems(two)).toBeVisible();
  await beat(two);
  await shot(two, "2-away");
  // And after a reload.
  await two.reload();
  await expect(syncChip(two)).toHaveAttribute("aria-label", /^CONFLICT · 1\./);
  await syncChip(two).click();
  await expect(problems(two)).toBeVisible();
  await beat(two);
  await shot(two, "3-after-reload");
  // Go to it brings the rename back, with what was typed and what was refused.
  await problems(two).getByRole("button", { name: "Go to it" }).click();
  await expect(two.getByTestId("conflict")).toBeVisible();
  await expect(renameOf(two, "n_onsite").getByRole("textbox")).toHaveValue(mine);
  await beat(two);
  await shot(two, "4-back");
  // Discard clears it.
  await syncChip(two).click();
  await problems(two).getByRole("button", { name: "Discard" }).click();
  await two.keyboard.press("Escape");
  await expect(renameOf(two, "n_onsite")).toHaveCount(0);
  await expect(nodeCard(two, "n_onsite").getByTestId("title")).toHaveText(theirs);
  await beat(two);
  await shot(two, "5-discarded");
  await context.close();
  await two.video()?.saveAs(join(out, "main-flow.webm"));
});

test("a rename that gets no answer is resent, then asks for Retry", { tag: "@server" }, async ({ page }) => {
  await openJourney(page, "server", "j_hiring");
  // Every request is cut: the server hears none of them.
  await page.route("**/api/journeys/*/patches", (route) => route.abort());
  await startRename(page, "n_offer", fresh("Offer"));
  await save(page, "n_offer");
  await expect(renameOf(page, "n_offer").getByTestId("write-retrying")).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./, { timeout: 30_000 });
  await syncChip(page).click();
  await expect(problems(page).getByRole("button", { name: "Retry" })).toBeVisible();
  await beat(page);
  await shot(page, "6-could-not-confirm");
});

test("an authoring edit the engine refuses is still at its field after a reload", { tag: "@server" }, async ({ page }) => {
  await newRoute(page, "server", routeName("Onboarding"));
  const kickoff = await addNode(page, "milestone", "Kickoff");
  await addNode(page, "milestone", "Launch");
  await formField(page, "final").getByRole("checkbox").check();
  await saveForm(page);
  // Kickoff is made final too: the engine allows one final milestone, and Save sends the edit anyway.
  await nodeCard(page, kickoff).getByTestId("card-open").click();
  await expect(structure(page)).toHaveAttribute("data-node", kickoff);
  await formField(page, "final").getByRole("checkbox").check();
  await nodeForm(page).getByTestId("node-form-save").click();
  const refused = formField(page, "final").locator('[data-testid="violation"][data-code="several_final_milestones"]');
  await expect(refused).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await beat(page);
  await shot(page, "7-authoring-refused");
  await page.reload();
  await expect(structure(page)).toHaveAttribute("data-node", kickoff);
  await expect(refused).toBeVisible();
  await expect(syncChip(page)).toHaveAttribute("aria-label", /^NOT SAVED · 1\./);
  await syncChip(page).click();
  await expect(problems(page)).toBeVisible();
  await beat(page);
  await shot(page, "8-authoring-after-reload");
});
