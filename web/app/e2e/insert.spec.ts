// Route insertion on cairn demo: the security-review route imported and published, then
// inserted into the vendor evaluation's Testing group (C19, B13, C14). The insertion starts
// after a host node and maps the route's reviewer onto the journey's existing findings
// reviewer, so the route's own filling decision is left out; review shows the incoming nodes
// and the wiring edge before anything applies, and the inserted root's Origin names the route and
// version. A second insertion's id is the service's test (insertion.rs).

import { expect, test } from "./test.ts";

import { goTo, openJourney, readable } from "./shell.ts";
import { apply } from "./proposals.ts";
import { publishRoute, startInsert, stepper } from "./insert.ts";

const JOURNEY = "j_vendor_eval";

test("C19, C14: a route inserted from a container, reviewed, applied, and shown in Origin", async ({ page }) => {
  await readable(page);
  await publishRoute(page);

  // After the access deliverable, with the reviewer mapped onto the journey's findings reviewer.
  await startInsert(page, JOURNEY, "n_testing", "Security review");
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

  // The inserted root's Details name the route and version.
  await openJourney(page, JOURNEY);
  await page.getByTestId("node-card").filter({ hasText: "Security review" }).first().getByTestId("card-open").click();
  await page.getByTestId("details").locator("summary").click();
  await expect(page.getByTestId("origin-route").first()).toHaveText("Security review, version 1");
});
