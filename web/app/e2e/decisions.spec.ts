// The decision view (C12) over the vendor evaluation and the hiring loop: each decision with
// its answer and what that answer affected, the partner decision gating the partner-led
// subset (and re-gating it live when the answer is revised), the server's projection shown
// row for row, and a decision opening its detail beside the view.
import { expect, test, type Page } from "@playwright/test";

import { nodePanel } from "./shell.ts";
import { openScreen, served } from "./views.ts";

function decisionRow(page: Page, node: string) {
  return page.locator(`[data-testid="decision-row"][data-node="${node}"]`);
}

/** Each affected node of `node`'s row with its relevance, by key. */
async function affected(page: Page, node: string): Promise<Record<string, string>> {
  const pairs = await decisionRow(page, node)
    .getByTestId("affected")
    .evaluateAll((items) => items.map((item) => [item.getAttribute("data-node") ?? "", item.getAttribute("data-relevance") ?? ""]));
  return Object.fromEntries(pairs) as Record<string, string>;
}

const PARTNER_LED = ["n_criteria", "n_partner_led", "n_partner_results"];

test("C12: the partner decision gates the partner-led subset, and revising it re-gates it", async ({ page }) => {
  await openScreen(page, "browser", "j_vendor_eval", "decisions", "n_partner_runs");
  await expect(decisionRow(page, "n_partner_runs").getByTestId("decision-answer")).toHaveText("no");
  expect(await affected(page, "n_partner_runs")).toEqual(Object.fromEntries(PARTNER_LED.map((key) => [key, "not_relevant"])));
  const panel = nodePanel(page, "n_partner_runs");
  await panel.getByRole("button", { name: "Revise the answer" }).click();
  await panel.getByTestId("answer-editor").getByLabel("Answer").selectOption("yes");
  await panel.getByRole("button", { name: "Save the answer" }).click();
  await expect(decisionRow(page, "n_partner_runs").getByTestId("decision-answer")).toHaveText("yes");
  await expect.poll(() => affected(page, "n_partner_runs")).toEqual(Object.fromEntries(PARTNER_LED.map((key) => [key, "relevant"])));
});

test("C12: every decision with its answer, pins, fills, and markers, as the server projects them", async ({ page }) => {
  await openScreen(page, "server", "j_vendor_eval", "decisions");
  const view = await served<{ decisions: { node: string; state: string; affects?: string[]; pins?: string; fills?: string }[]; edges: unknown[] }>(
    page,
    "j_vendor_eval",
    "decisions",
  );
  const rows = page.getByTestId("decision-row");
  await expect(rows).toHaveCount(view.decisions.length);
  for (const entry of view.decisions) {
    const row = decisionRow(page, entry.node);
    await expect(row).toHaveAttribute("data-state", entry.state);
    expect(Object.keys(await affected(page, entry.node))).toEqual(entry.affects ?? []);
    await expect(row.getByTestId("pins")).toHaveCount(entry.pins === undefined ? 0 : 1);
    await expect(row.getByTestId("fills")).toHaveCount(entry.fills === undefined ? 0 : 1);
  }
  await expect(page.locator("[data-testid=canvas] [data-testid=node-card]")).toHaveCount(view.decisions.length);
  await expect(page.locator("[data-testid=canvas] [data-testid=edge-line]")).toHaveCount(view.edges.length);
});

test("C12: a decision's card opens its detail beside the view, and closing it stays on the view", async ({ page }) => {
  await openScreen(page, "browser", "j_hiring", "decisions");
  await expect.poll(async () => Object.keys(await affected(page, "n_make_offer"))).toEqual(["n_close_out", "n_offer"]);
  await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
  const panel = nodePanel(page, "n_make_offer");
  await expect(panel).toBeVisible();
  await expect(page).toHaveURL(/#\/journeys\/j_hiring\/decisions\/nodes\/n_make_offer$/);
  await expect(decisionRow(page, "n_make_offer")).toHaveAttribute("data-selected", "true");
  await panel.getByRole("link", { name: "Close the node detail" }).click();
  await expect(page).toHaveURL(/#\/journeys\/j_hiring\/decisions$/);
  await expect(page.getByTestId("decision-table")).toBeVisible();
});
