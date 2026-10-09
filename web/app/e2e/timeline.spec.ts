// The timeline (C13, with F6 and F7's presentation): the product launch's late code freeze
// marked short, with the chain that explains it; and a pin putting a node on the timeline. The
// server host draws it from the page's own derivation of the server's document, which
// web/wasm's agreement cases hold to the server's projection.
import { expect, test, type Page } from "@playwright/test";

import { pin } from "./detail.ts";
import { nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

function entry(page: Page, node: string) {
  return page.locator(`[data-testid="timeline-entry"][data-node="${node}"]`);
}

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

test("C13: a pin puts a node on the timeline", async ({ page }) => {
  await openAt(page, "browser", "j_hiring", "plan/timeline", { node: "n_offer", fixedToday: FIXED_TODAY });
  await pin(nodePanel(page, "n_offer"), "2026-10-20");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-origin", "pin");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-date", "2026-10-20");
  await expect(entry(page, "n_offer")).toHaveAttribute("data-selected", "true");
});
