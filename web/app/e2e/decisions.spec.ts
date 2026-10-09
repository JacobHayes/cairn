// The decision view (C12), PLAN, GRAPH with DECISIONS on, over the vendor evaluation and the
// hiring loop: the partner decision gating the partner-led subset (and re-gating it live when
// the answer is revised), shown in the decision's own detail as what its answer affects, and a
// decision's card opening that detail beside the view. The server host draws the view from the
// page's own derivation of the server's document, which web/wasm's agreement cases hold to the
// server's projection.
import { expect, test, type Page } from "@playwright/test";

import { nodeCard, nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

/** Each node `node`'s answer affects, with its relevance, by key. */
async function affected(page: Page, node: string): Promise<Record<string, string>> {
  const pairs = await nodePanel(page, node)
    .getByTestId("decision-affects")
    .getByTestId("affected")
    .evaluateAll((items) => items.map((item) => [item.getAttribute("data-node") ?? "", item.getAttribute("data-relevance") ?? ""]));
  return Object.fromEntries(pairs) as Record<string, string>;
}

const PARTNER_LED = ["n_criteria", "n_partner_led", "n_partner_results"];

test("C12: the partner decision gates the partner-led subset, and revising it re-gates it", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/graph?decisions=1", { node: "n_partner_runs", fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("canvas")).toBeVisible();
  await expect(nodePanel(page, "n_partner_runs").getByRole("radio", { name: /^no\b/i })).toBeChecked();
  await expect.poll(() => affected(page, "n_partner_runs")).toEqual(Object.fromEntries(PARTNER_LED.map((key) => [key, "not_relevant"])));
  const panel = nodePanel(page, "n_partner_runs");
  await panel.getByRole("radio", { name: /^yes\b/i }).check();
  await panel.getByRole("button", { name: "Save change" }).click();
  await expect(panel.getByRole("radio", { name: /^yes\b/i })).toBeChecked();
  await expect.poll(() => affected(page, "n_partner_runs")).toEqual(Object.fromEntries(PARTNER_LED.map((key) => [key, "relevant"])));
});

test("C12: a decision's card opens its detail beside the view, and closing it stays on the view", async ({ page }) => {
  await openAt(page, "browser", "j_hiring", "plan/graph?decisions=1", { fixedToday: FIXED_TODAY });
  await nodeCard(page, "n_make_offer").getByTestId("card-open").click();
  const panel = nodePanel(page, "n_make_offer");
  await expect(panel).toBeVisible();
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/plan\/graph\/nodes\/n_make_offer\?decisions=1$/);
  await expect.poll(async () => Object.keys(await affected(page, "n_make_offer"))).toEqual(["n_close_out", "n_offer"]);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await expect(page).toHaveURL(/\/journeys\/j_hiring\/plan\/graph\?decisions=1$/);
  await expect(page.getByTestId("canvas")).toBeVisible();
});
