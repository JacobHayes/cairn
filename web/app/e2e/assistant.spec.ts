// Brief 5.8 in the browser: the assistant panel on the server host against the demo's
// scripted model. A direct change is applied and reported with links to the node it wrote, and a
// breakdown asked for comes back as a proposal that opens in proposal review and applies
// (I5's two paths, I7: applying is the user's click); the conversation is the user's per
// target, read back after a reload, on a route's draft too. The test is tagged @server:
// the demo's scripted model is one for the whole server, so it runs one at a time with the
// other tests that write to it. The conversation's own rules are vitest's.
import { expect, test } from "@playwright/test";

import { startVendorJourney } from "./acting.ts";
import { ask, openPanel, patchId, script } from "./assistant.ts";
import { state } from "./detail.ts";
import { confirmAndApply, reviewOpen } from "./proposals.ts";
import { nodePanel, open, openJourney } from "./shell.ts";

test("I5, I7: a direct change reported with its node, and a breakdown proposed, reviewed, and applied", { tag: "@server" }, async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openJourney(page, "server", journey);
  const panel = await openPanel(page);
  await expect(panel.getByTestId("assistant-empty")).toBeVisible();

  await script(page, [
    { calls: [{ name: "transition_node", arguments: { journey, node: "n_kickoff", transition: "reach", patch_id: patchId(), base_revision: 1 } }] },
    { say: "Kickoff is marked reached." },
  ]);
  await ask(page, "Kickoff happened this morning.");
  const applied = panel.getByTestId("assistant-applied");
  await expect(applied).toHaveCount(1);
  await expect(panel.getByTestId("assistant-reply")).toContainText("Kickoff is marked reached.");
  await applied.locator('[data-testid="assistant-node"][data-node="n_kickoff"]').click();
  await expect(state(nodePanel(page, "n_kickoff"))).toHaveAttribute("data-status", "done");
  // Opening the node brought the inspector tab forward; the conversation is a click back.
  await expect(panel).toBeHidden();
  await page.getByTestId("tab-assistant").click();
  await expect(panel).toBeVisible();

  const pieces = ["Ingest workload", "Query workload"].map((title, index) => ({
    op: "add_node",
    node: { key: `n_piece_${String(index)}`, id: `piece-${String(index)}`, kind: "deliverable", title, parent: "n_workload" },
  }));
  await script(page, [
    { calls: [{ name: "apply_patch", arguments: { patch: { id: patchId(), target: { journey }, base_revision: 2, mutations: pieces } } }] },
    { say: "I drafted the breakdown for your review." },
  ]);
  await ask(page, "Break the test workload down into an ingest and a query workload.");
  const proposed = panel.getByTestId("assistant-proposed");
  await expect(proposed).toHaveAttribute("data-because", "structural");
  await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]')).toHaveCount(0);

  await proposed.getByTestId("review-proposal").click();
  await reviewOpen(page);
  await page.getByTestId("projection-list").click();
  await expect(page.locator('[data-testid="diff-node"][data-status="add"]')).toHaveCount(2);
  await confirmAndApply(page);
  await page.getByTestId("applied-journey").click();
  for (const title of ["Ingest workload", "Query workload"]) {
    await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]').filter({ hasText: title })).toHaveCount(1);
  }
  // Back on the journey, the panel is still open, with both turns' writes.
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-applied")).toHaveCount(1);
  await expect(page.getByTestId("assistant-panel").getByTestId("review-proposal")).toHaveCount(1);

  // The conversation is kept per target: read back after a reload, and a route's draft has its own.
  await page.reload();
  const kept = page.getByTestId("assistant-panel");
  await expect(kept.getByTestId("assistant-said")).toHaveText(["Kickoff happened this morning.", "Break the test workload down into an ingest and a query workload."]);
  await expect(kept.getByTestId("assistant-reply").last()).toContainText("I drafted the breakdown");
  await open(page, "server", "/routes/product-launch");
  await expect(page.getByTestId("assistant-panel")).toHaveAttribute("data-target", "route:product-launch");
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveCount(0);
});
