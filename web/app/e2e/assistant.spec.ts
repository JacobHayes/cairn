// Brief 5.8 in the browser: the assistant panel on the server host against the demo's
// scripted model. A direct change is applied and reported with links to the node it wrote, and a
// breakdown asked for comes back as a proposal that opens in proposal review and applies
// (I5's two paths, I7: applying is the user's click); the conversation is the user's per
// target, read back after a reload, on a route's draft too. The test is tagged @server:
// the demo's scripted model is one for the whole server, so it runs one at a time with the
// other tests that write to it. The conversation's own rules are vitest's.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { startVendorJourney } from "./acting.ts";
import { ask, openPanel, patchId, script, type ScriptStep } from "./assistant.ts";
import { state } from "./detail.ts";
import { apply, reviewOpen } from "./proposals.ts";
import { derived, goTo, nodePanel, open, openJourney, whenSettled } from "./shell.ts";

/** A model's step that notes `node` and completes it, as one direct change. */
function noteAndComplete(journey: string, node: string, base: number): ScriptStep {
  const mutations = [
    { op: "add_annotation", annotation: { key: "a_done", node, note: "Held this morning." } },
    { op: "transition", node, transition: "complete" },
  ];
  return { calls: [{ name: "update_journey_state", arguments: { journey, patch_id: patchId(), base_revision: base, mutations } }] };
}

/**
 * A new tab holds none of this tab's turns: it shows the conversation as Cairn kept it, the
 * write reading `said`, what the live card said, and linking the node it wrote.
 */
async function keptInANewTab(page: Page, journey: string, said: string): Promise<void> {
  const fresh = await page.context().newPage();
  await openJourney(fresh, "server", journey);
  const report = (await openPanel(fresh)).getByTestId("assistant-report").first();
  await expect(report).toHaveText(said);
  await expect(report.locator('[data-testid="assistant-node"][data-node="n_kickoff"]')).toHaveCount(1);
  await fresh.close();
}

test("I5, I7: a direct change reported with its node, and a breakdown proposed, reviewed, and applied", { tag: ["@server", "@serial"] }, async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openJourney(page, "server", journey);
  const panel = await openPanel(page);
  await expect(panel.getByTestId("assistant-empty")).toBeVisible();

  await script(page, [noteAndComplete(journey, "n_kickoff", 1), { say: "Kickoff is marked done." }]);
  await ask(page, "Kickoff happened this morning.");
  const applied = panel.getByTestId("assistant-applied");
  await expect(applied).toHaveCount(1);
  const said = (await whenSettled(page, () => applied.textContent())) ?? "";
  await expect(applied.locator('[data-testid="assistant-node"][data-node="n_kickoff"]')).toHaveText("Kickoff");
  await expect(panel.getByTestId("assistant-reply")).toContainText("Kickoff is marked done.");
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
    { calls: [{ name: "apply_patch", arguments: { target: { journey }, patch_id: patchId(), base_revision: 2, mutations: pieces } }] },
    { say: "I drafted the breakdown for your review." },
  ]);
  await ask(page, "Break the test workload down into an ingest and a query workload.");
  const proposed = panel.getByTestId("assistant-proposed");
  await expect(proposed).toHaveAttribute("data-because", "structural");
  await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]')).toHaveCount(0);

  await proposed.getByTestId("review-proposal").click();
  await reviewOpen(page);
  await goTo(page, "list");
  await expect(page.locator('[data-testid="list-row"][data-diff="added"]')).toHaveCount(2);
  await apply(page);
  await goTo(page, "graph");
  for (const title of ["Ingest workload", "Query workload"]) {
    await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]').filter({ hasText: title })).toHaveCount(1);
  }
  // Back on the journey, the panel is still open, with both turns' writes.
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-applied")).toHaveCount(1);
  await expect(page.getByTestId("assistant-panel").getByTestId("review-proposal")).toHaveCount(1);

  // The conversation is kept per target: read back after a reload, and a route's draft has its own.
  // Only a person brings the assistant forward: the tab they left is the one a reload or a phone's width keeps.
  const inspector = page.getByTestId("tab-inspector");
  await inspector.click();
  await expect(inspector).toHaveAttribute("aria-selected", "true");
  await page.reload();
  await derived(page);
  await expect(page.getByTestId("assistant-panel")).toHaveCount(1);
  await expect(inspector).toHaveAttribute("aria-selected", "true");
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByTestId("canvas")).toBeVisible();
  await expect(inspector).toHaveAttribute("aria-selected", "true");
  await page.setViewportSize({ width: 1280, height: 720 });
  const kept = page.getByTestId("assistant-panel");
  await expect(kept.getByTestId("assistant-said")).toHaveText(["Kickoff happened this morning.", "Break the test workload down into an ingest and a query workload."]);
  await expect(kept.getByTestId("assistant-reply").last()).toContainText("I drafted the breakdown");
  await keptInANewTab(page, journey, said);
  await open(page, "server", "/routes/product-launch");
  await expect(page.getByTestId("assistant-panel")).toHaveAttribute("data-target", "route:product-launch");
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveCount(0);
});
