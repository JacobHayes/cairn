// Brief 7.7 on the in-browser host: the security-review segment imported and published, then
// inserted into the vendor evaluation's Testing group (C19, B13, C14). The insertion starts
// after a host node and maps the segment's reviewer onto the journey's existing findings
// reviewer, so the segment's own filling decision is left out; review shows the incoming nodes
// and the wiring edge before anything applies, and a member's Origin names the segment and
// version. A second insertion's id, where a segment is used and its newer versions are the
// service's tests (insertion.rs).

import { expect, test } from "./test.ts";

import { goTo, menuItem, openJourney } from "./shell.ts";
import { apply } from "./proposals.ts";
import { publishSegment, startInsert, stepper } from "./segments.ts";

const JOURNEY = "j_vendor_eval";

test("C19, C14: a segment inserted from a container, reviewed, applied, and shown in Origin", async ({ page }) => {
  await publishSegment(page, "browser");

  // After the access deliverable, with the reviewer mapped onto the journey's findings reviewer.
  await startInsert(page, "browser", JOURNEY, "n_testing", "Security review");
  await expect(stepper(page).getByLabel("Under")).toHaveValue("n_testing");
  await stepper(page).getByLabel("Add to Starts after").selectOption("n_access");
  // The incoming root is drawn on the graph as review will draw it.
  await expect(page.locator('[data-testid="node-card"][data-diff="added"]').first()).toBeVisible();
  await stepper(page).getByTestId("insert-next").click();
  await stepper(page).getByLabel("Reviewer becomes").selectOption("r_findings_reviewer");
  await stepper(page).getByTestId("insert-review").click();
  await expect(page.getByTestId("proposal-rail")).toBeVisible();
  await goTo(page, "list");
  // Three nodes come in (Who reviews? is left out), marked as added, wired after the access deliverable.
  await expect(page.locator('[data-testid="list-row"][data-diff="added"]')).toHaveCount(3);
  await expect(page.getByTestId("changes-fold")).toContainText("Environment access");
  await apply(page);

  // A member's Details name the segment and version.
  await openJourney(page, "browser", JOURNEY);
  await page.getByTestId("node-card").filter({ hasText: "Threat model" }).first().getByTestId("card-open").click();
  await page.getByTestId("details").locator("summary").click();
  await expect(page.getByTestId("segment-origin").first()).toHaveText("From segment Security review, version 1.");
  // Show insertion selects the insertion's root.
  await menuItem(page.getByTestId("node-detail"), "show-insertion");
  await expect(page.getByTestId("node-detail")).toHaveAttribute("aria-label", "Security review");
});
