// One address per journey and node (C2, C9, C11, C16, E4): Home at `/`, the pages each path names and
// the ones that are gone, the rail's current item, a selected node drawn whatever the view hides, and the
// walk keeping its address. They run on the in-browser host, whose fixtures hold the same journeys.
import { expect, test } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { expectAddress, goTo, nodeCard, nodePanel, open, openAt, visit } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

test("E4, C16: Home lists what is waiting on the viewer by journey; a row opens the node's own address on the graph, and a row action acts in place", async ({ page }) => {
  await visit(page, "browser", "/");
  const journey = page.getByTestId("home-journey").first();
  await expect(journey).toBeVisible();
  const id = (await journey.getAttribute("data-journey")) ?? "";
  // The journey is also in the table of the viewer's active journeys below.
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toBeVisible();
  const item = journey.getByTestId("home-item").first();
  const node = (await item.getAttribute("data-node")) ?? "";
  await item.locator(".node-row-main").first().click();
  await expectAddress(page, `/journeys/${id}/nodes/${node}`);
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  await expect(nodePanel(page, node)).toBeVisible();
  await expect(nodeCard(page, node)).toBeVisible();
});

test("C16: a row action on Home writes in place and the address stays `/`", async ({ page }) => {
  await visit(page, "browser", "/");
  const start = page.getByTestId("home-item").getByRole("button", { name: "Start" }).first();
  await expect(start).toBeVisible();
  await start.click();
  await expect(page.getByTestId("home")).toBeVisible();
  expect(new URL(page.url()).pathname + new URL(page.url()).search).toBe("/");
});

test("the rail lights Home at `/` only, Journeys on the journey pages and their proposals, Routes on the route pages and theirs, and Entities on its page and its proposals", async ({ page }) => {
  const rail = page.getByRole("navigation", { name: "Screens", exact: true });
  const current = rail.locator("[aria-current=page]");
  const cases: [string, string][] = [
    ["/", "Home"],
    ["/journeys", "Journeys"],
    ["/journeys?status=any", "Journeys"],
    ["/journeys/new", "Journeys"],
    ["/journeys/j_vendor_eval", "Journeys"],
    ["/journeys/j_vendor_eval/nodes/n_plan", "Journeys"],
    ["/journeys/j_vendor_eval/proposals", "Journeys"],
    ["/routes", "Routes"],
    ["/routes/vendor-evaluation/proposals", "Routes"],
    ["/entities", "Entities"],
    ["/entities/proposals", "Entities"],
    ["/routes/new", "Routes"],
    ["/routes/vendor-evaluation", "Routes"],
  ];
  for (const [address, item] of cases) {
    await visit(page, "browser", address);
    await expect(current, address).toHaveText([item]);
  }
});

test("a bare journey opens the Summary, a node or an edge opens the Graph, and the paths that are gone (a route's versions and draft pages among them) are no page", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_vendor_eval");
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "summary");
  await visit(page, "browser", "/journeys/j_vendor_eval/nodes/n_plan");
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  await visit(page, "browser", "/journeys/j_vendor_eval/edges/n_access~n_plan");
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  for (const gone of ["/proposals/pr_gone", "/proposals/pr_gone/nodes/n_plan", "/mine", "/new", "/library", "/journeys/j_vendor_eval/graph", "/journeys/j_vendor_eval/summary", "/journeys/j_vendor_eval/walk/nodes/n_plan", "/routes/vendor-evaluation/versions", "/routes/vendor-evaluation/draft", "/routes/vendor-evaluation/draft/nodes/n_plan"]) {
    await visit(page, "browser", gone);
    await expect(page.getByTestId("no-such-page"), gone).toBeVisible();
    expect(new URL(page.url()).pathname, gone).toBe(gone);
  }
});

test("C14: a proposal that is not there links somewhere that is: its journey's open proposals, or the journeys when the journey is not there either", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_vendor_eval/proposals/pr_gone");
  await page.getByTestId("no-such-proposal").getByRole("link").click();
  await expectAddress(page, "/journeys/j_vendor_eval/proposals");
  await expect(page.getByTestId("proposals-pane")).toBeVisible();
  await visit(page, "browser", "/journeys/j_gone/proposals/pr_gone");
  await page.getByTestId("no-such-proposal").getByRole("link").click();
  await expectAddress(page, "/journeys");
  // An absent journey's open proposals fail closed like the journey: nothing would create it, so no tab.
  await visit(page, "browser", "/journeys/j_gone/proposals");
  await expect(page.getByTestId("journey-missing")).toBeVisible();
  await expect(page.getByTestId("proposals-pane")).toHaveCount(0);
});

test("C4: a selected node is drawn with its container however the view is set, the address is as opened, and closing the container selects it", async ({ page }) => {
  const opened = "/journeys/j_vendor_eval/nodes/n_access?view=graph&depth=stages&q=nothing-is-called-this";
  await open(page, "browser", opened);
  await expect(page.getByTestId("journey-frame")).toBeVisible();
  await expect(nodeCard(page, "n_access")).toBeVisible();
  await expect(nodeCard(page, "n_setup")).toBeVisible();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Stages");
  await expect(page.getByTestId("toolbar-search")).toHaveValue("nothing-is-called-this");
  await expectAddress(page, opened);
  // Closing the container that holds the selection moves the selection to it.
  await nodeCard(page, "n_setup").getByTestId("card-collapse").dispatchEvent("click");
  await expectAddress(page, "/journeys/j_vendor_eval/nodes/n_setup?depth=stages&q=nothing-is-called-this&shut=n_setup");
  await expect(nodeCard(page, "n_access")).toHaveCount(0);
});

test("C9: a selected node is a row in the list and the timeline whatever the filter hides, and folding its container selects the container", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list?q=nothing-is-called-this", { node: "n_access", fixedToday: FIXED_TODAY });
  const row = page.locator('[data-testid="list-row"][data-node="n_access"]');
  await expect(row).toBeVisible();
  await expect(row).toHaveAttribute("aria-selected", "true");
  await goTo(page, "timeline");
  await expect(page.locator('[data-node="n_access"]').first()).toBeVisible();
});

test("C11: the walk keeps the address it was opened at: the card is the one editor", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Walk"), { route: "vendor-evaluation", version: 1 });
  await expect(page.getByTestId("triage")).toHaveAttribute("data-order", /\S/);
  const first = ((await page.getByTestId("triage").getAttribute("data-order")) ?? "").split(" ")[0] ?? "";
  await open(page, "browser", `/journeys/${id}/nodes/${first}?view=walk&depth=decisions`);
  await expect(page.getByTestId("triage")).toBeVisible();
  await expectAddress(page, `/journeys/${id}/nodes/${first}?view=walk&depth=decisions`);
  // The card is the node's one editor; the inspector does not open a second one.
  await expect(page.getByTestId("node-detail")).toHaveCount(1);
  await expect(page.getByTestId("triage").getByTestId("node-detail")).toHaveAttribute("data-node", first);
});
