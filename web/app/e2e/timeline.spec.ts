// The timeline (C13, with F6 and F7's presentation): the vendor evaluation anchored on its
// final milestone, the decision meeting; the product launch's late code freeze marked short,
// with the chain that explains it; and a journey with no final milestone drawn with no anchor
// once it has a date. The server host draws it from the page's own derivation of the server's
// document, which web/wasm's agreement cases hold to the server's projection.
import { expect, test, type Page } from "@playwright/test";

import { pin } from "./detail.ts";
import { nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

function entry(page: Page, node: string) {
  return page.locator(`[data-testid="timeline-entry"][data-node="${node}"]`);
}

/** The timeline's rows as [node, date, origin], in their order. */
function rows(page: Page): Promise<string[][]> {
  return page
    .getByTestId("timeline-entry")
    .evaluateAll((items) => items.map((item) => ["data-node", "data-date", "data-origin"].map((name) => item.getAttribute(name) ?? "")));
}

test("C13: the vendor evaluation's timeline ends at the decision meeting, its final milestone", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/timeline", { fixedToday: FIXED_TODAY });
  const timeline = page.getByTestId("timeline");
  await expect(timeline).toHaveAttribute("data-end", "n_decision_meeting");
  await expect(timeline).toHaveAttribute("data-end-date", "2026-11-20");
  await expect(entry(page, "n_decision_meeting")).toHaveAttribute("data-end", "true");
  await expect(page.locator('[data-testid="timeline-entry"][data-end="true"]')).toHaveCount(1);
  await expect(entry(page, "n_kickoff")).toHaveAttribute("data-origin", "actual");
  await expect(entry(page, "n_final_report")).toHaveAttribute("data-origin", "pin");
  await expect(entry(page, "n_review_opens")).toHaveAttribute("data-origin", "due");
  const dates = (await rows(page)).map(([, date]) => date ?? "");
  expect([...dates].sort()).toEqual(dates);
});

test("C13, F6, F7: the late code freeze is marked short, and its row says why", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "plan/timeline", { fixedToday: FIXED_TODAY });
  const freeze = entry(page, "n_code_freeze");
  await expect(freeze).toHaveAttribute("data-origin", "actual");
  await expect(freeze).toHaveAttribute("data-shortfall", "2");
  await expect(entry(page, "n_launch")).toHaveAttribute("data-shortfall", "2");
  await expect(entry(page, "n_launch")).toHaveAttribute("data-end", "true");
  await freeze.getByRole("button", { name: "Why" }).click();
  const why = freeze.getByTestId("timeline-why");
  const short = why.getByTestId("shortfall");
  await expect(short).toHaveAttribute("data-days", "2");
  await expect(short.getByTestId("chain-link").first()).toBeVisible();
  await expect(page.getByTestId("timeline-undated").locator("a")).toHaveText(["Retrospective"]);
});

test("C13: with no final milestone there is no end anchor; a pin puts a node on the timeline", async ({ page }) => {
  await openAt(page, "browser", "j_hiring", "plan/timeline", { node: "n_offer", fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("timeline-empty")).toBeVisible();
  await pin(nodePanel(page, "n_offer"), "2026-10-20");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-origin", "pin");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-date", "2026-10-20");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-selected", "true");
  await expect(page.getByTestId("timeline")).toHaveAttribute("data-end", "");
  await expect(page.locator('[data-testid="timeline-entry"][data-end="true"]')).toHaveCount(0);
});

