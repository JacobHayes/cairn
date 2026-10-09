// Brief 5.8 in the browser: the assistant panel on the server host against 4.4's scripted
// provider. A direct change is applied and reported with links to the node it wrote, and a
// breakdown asked for comes back as a proposal that opens in proposal review and applies
// (I5's two paths, I7: applying is the user's click); the conversation is the user's per
// target, read back on the overview's panel and after a reload, on a route's draft too, with
// a message being typed kept to its target and a turn that outlives the panel closing; and
// the in-browser host never shows the panel (capabilities gating). The tests on the server
// host are tagged @server: the fixture server's scripted model is one for the whole server, so
// they run one at a time with the other tests that write to it.
import { expect, test } from "@playwright/test";

import { startVendorJourney } from "./acting.ts";
import { ask, openPanel, patchId, script } from "./assistant.ts";
import { openOverview } from "./around.ts";
import { state } from "./detail.ts";
import { confirmAndApply, reviewOpen } from "./proposals.ts";
import { goWithin, nodePanel, open, openJourney } from "./shell.ts";

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
  await expect(page.locator('[data-testid="diff-node"][data-status="added"]')).toHaveCount(2);
  await confirmAndApply(page);
  await page.getByTestId("applied-journey").click();
  for (const title of ["Ingest workload", "Query workload"]) {
    await expect(page.locator('[data-testid="node-card"][data-parent="n_workload"]').filter({ hasText: title })).toHaveCount(1);
  }
  // Back on the journey, the panel is still open, with both turns' writes.
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-applied")).toHaveCount(1);
  await expect(page.getByTestId("assistant-panel").getByTestId("review-proposal")).toHaveCount(1);
});

test("I5: the conversation is kept per target, read back on the overview and after a reload, a route's draft its own", { tag: "@server" }, async ({ page }) => {
  const journey = await startVendorJourney(page);
  await openOverview(page, "server", journey);
  const panel = await openPanel(page);
  await expect(panel).toHaveAttribute("data-target", `journey:${journey}`);
  await script(page, [{ say: "Nothing is overdue yet; the plan comes first." }]);
  await ask(page, "What should I do first?");
  await expect(panel.getByTestId("assistant-reply")).toHaveCount(1);

  await page.reload();
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveText("What should I do first?");
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-reply")).toContainText("the plan comes first");

  await open(page, "server", "/routes/product-launch");
  await expect(page.getByTestId("assistant-panel")).toHaveAttribute("data-target", "route:product-launch");
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveCount(0);
});

test("capabilities gating: the in-browser host never shows the panel", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("assistant-toggle")).toHaveCount(0);
  await openOverview(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("overview")).toBeVisible();
  await expect(page.getByTestId("assistant-toggle")).toHaveCount(0);
  await openJourney(page, "server", "j_vendor_eval");
  await expect(page.getByTestId("assistant-toggle")).toHaveCount(1);
});

test("I5: a message typed about one journey stays with it, and a turn outlives the panel closing", { tag: "@server" }, async ({ page }) => {
  const first = await startVendorJourney(page);
  const second = await startVendorJourney(page);
  await openJourney(page, "server", second);
  await goWithin(page, `/journeys/${first}`);
  await expect(page.getByTestId("journey-name")).toBeVisible();
  const panel = await openPanel(page);
  await panel.getByTestId("assistant-message").fill("About the first journey.");
  // Back to a journey the tab already holds, so the page does not pass through loading.
  await goWithin(page, `/journeys/${second}`);
  await expect(page.getByTestId("assistant-panel")).toHaveAttribute("data-target", `journey:${second}`);
  await expect(page.getByTestId("assistant-message")).toHaveValue("");

  await script(page, [{ say: "Answered slowly.", after_ms: 3000 }]);
  await page.getByTestId("assistant-message").fill("Take your time.");
  await page.getByTestId("assistant-send").click();
  await expect(page.getByTestId("assistant-working")).toBeVisible();
  await page.getByRole("button", { name: "Close the assistant" }).click();
  await page.getByTestId("assistant-toggle").click();
  await expect(page.getByTestId("assistant-working")).toBeVisible();
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-reply")).toContainText("Answered slowly.");
  await expect(page.getByTestId("assistant-panel").getByTestId("assistant-said")).toHaveCount(1);
});
