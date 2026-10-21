// One address per journey and node (C2, C9, C14, C4): a proposal or journey that is gone links somewhere
// that is, and a selected node is drawn whatever the view hides. The address-to-page map and the
// rail's current item are the router's (src/address.test.ts, src/journeys/address.test.ts). They run
// on the in-browser host, whose fixtures hold the same journeys.
import { checkpoint, expect, test } from "./test.ts";

import { expectAddress, goTo, nodeCard, nodePanel, open, visit } from "./shell.ts";

test("C14: a proposal that is not there links somewhere that is: its journey's open proposals, or the journeys when the journey is not there either", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_vendor_eval/proposals/pr_gone");
  await expect(page.getByTestId("layer-band")).toHaveAttribute("data-mode", "missing");
  await page.getByTestId("open-proposals").click();
  await expectAddress(page, "/journeys/j_vendor_eval/proposals");
  await expect(page.getByTestId("proposals-pane")).toBeVisible();
  await open(page, "browser", "/journeys/j_gone/proposals/pr_gone");
  await expect(page.getByTestId("journey-missing")).toBeVisible();
  // An absent journey's open proposals fail closed like the journey: nothing would create it, so no tab.
  await open(page, "browser", "/journeys/j_gone/proposals");
  await expect(page.getByTestId("journey-missing")).toBeVisible();
  await expect(page.getByTestId("proposals-pane")).toHaveCount(0);
  // A path that is gone is no page.
  await open(page, "browser", "/mine");
  await expect(page.getByTestId("no-such-page")).toBeVisible();
  await checkpoint(page);
});

test("C4: a selected node is a row in the list and the timeline, and drawn with its container on the graph, however the view is set; closing the container selects it", async ({ page }) => {
  const opened = "/journeys/j_vendor_eval/nodes/n_access?view=graph&depth=stages&q=nothing-is-called-this";
  await open(page, "browser", opened);
  await expect(page.getByTestId("journey-frame")).toBeVisible();
  await expect(nodeCard(page, "n_access")).toBeVisible();
  await expect(nodeCard(page, "n_setup")).toBeVisible();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Stages");
  await expect(page.getByTestId("toolbar-search")).toHaveValue("nothing-is-called-this");
  await expectAddress(page, opened);
  // The search matches nothing, yet the selected node is a row in the list and an entry in the timeline.
  await goTo(page, "list");
  const row = page.locator('[data-testid="list-row"][data-node="n_access"]');
  await expect(row).toBeVisible();
  await expect(row).toHaveAttribute("aria-selected", "true");
  await goTo(page, "timeline");
  await expect(page.locator('[data-node="n_access"]').first()).toBeVisible();
  await goTo(page, "graph");
  // Closing the container that holds the selection moves the selection to it.
  await nodeCard(page, "n_setup").getByTestId("card-collapse").dispatchEvent("click");
  await expectAddress(page, "/journeys/j_vendor_eval/nodes/n_setup?depth=stages&q=nothing-is-called-this&shut=n_setup");
  await expect(nodeCard(page, "n_access")).toHaveCount(0);
});
