// Brief 7.7 on the in-browser host: the security-review segment imported and published, then
// inserted twice into the vendor evaluation's Testing group (C19, B13, C14). The first
// insertion starts after a host node and maps the segment's reviewer onto the journey's
// existing findings reviewer, so the segment's own filling decision is left out; review shows
// the incoming nodes and the wiring edge before anything applies. The second makes it used in
// two places. Then the Library lists the segment as used in two places, its detail lists both
// insertions, a member's Origin names the segment and version, and a published version 2 shows
// as available there.
import { expect, test, type Page } from "@playwright/test";

import { menuItem, open, openJourney } from "./shell.ts";
import { publishDraft, publishSegment, startInsert, stepper } from "./segments.ts";

const JOURNEY = "j_vendor_eval";

/** Confirms the review and applies the proposal. */
async function apply(page: Page): Promise<void> {
  await page.getByTestId("reviewed").check();
  await page.getByTestId("apply-proposal").click();
  await expect(page.getByTestId("proposal-status")).toHaveAttribute("data-status", "applied");
}

test("C19, C14: a segment inserted twice from a container, reviewed, applied, listed, and shown in Origin", async ({ page }) => {
  await publishSegment(page, "browser");

  // First: after the access deliverable, with the reviewer mapped onto the journey's findings reviewer.
  await startInsert(page, "browser", JOURNEY, "n_testing", "Security review");
  await expect(stepper(page).getByLabel("Under")).toHaveValue("n_testing");
  await stepper(page).getByLabel("Add to Starts after").selectOption("n_access");
  // The incoming root is drawn on the graph as review will draw it.
  await expect(page.getByTestId("card-mark").filter({ hasText: "Add" }).first()).toBeVisible();
  await stepper(page).getByTestId("insert-next").click();
  await stepper(page).getByLabel("Reviewer becomes").selectOption("r_findings_reviewer");
  await stepper(page).getByTestId("insert-review").click();
  await expect(page.getByTestId("proposal")).toBeVisible();
  await page.getByTestId("projection-list").click();
  // Three nodes come in (Who reviews? is left out), outlined as the segment's version 1, wired after the access deliverable.
  await expect(page.getByTestId("diff-node").filter({ hasText: "⧉ Security review v1" })).toHaveCount(3);
  await expect(page.getByTestId("diff-edge")).toContainText("now requires Environment access");
  await apply(page);

  // Again: the root's id is taken, so it takes the suffix.
  await startInsert(page, "browser", JOURNEY, "n_testing", "Security review");
  await stepper(page).getByTestId("insert-next").click();
  await stepper(page).getByTestId("insert-review").click();
  await apply(page);

  // The Library and the segment's detail say where it is used.
  await open(page, "browser", "/library?type=segments");
  const row = page.locator('[data-testid="route-row"][data-route="security-review"]');
  await expect(row.getByTestId("segment-use")).toHaveText("Used in 2 places");
  await row.getByRole("link", { name: "Security review" }).click();
  await expect(page.locator('[data-testid="version"][data-version="1"]').getByTestId("insertion-use")).toHaveCount(2);

  // A member's Origin names the segment and version; a published version 2 shows as available.
  await openJourney(page, "browser", JOURNEY);
  await page.getByTestId("node-card").filter({ hasText: "Threat model" }).first().getByTestId("card-open").click();
  await page.getByTestId("origin").locator("summary").click();
  await expect(page.getByTestId("segment-origin").first()).toHaveText("From segment Security review, version 1.");
  // Show insertion selects the insertion's root.
  await menuItem(page.getByTestId("node-detail"), "show-insertion");
  await expect(page.getByTestId("node-detail")).toHaveAttribute("aria-label", "Security review");
  await open(page, "browser", "/routes/security-review/versions");
  await publishDraft(page);
  await openJourney(page, "browser", JOURNEY);
  await page.getByTestId("node-card").filter({ hasText: "Threat model" }).first().getByTestId("card-open").click();
  await expect(page.getByTestId("segment-origin").nth(1)).toHaveText("Version 2 is available.");
});
