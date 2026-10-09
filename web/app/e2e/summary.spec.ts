// The Summary page (C18): the journey card's way to it, a node opening beside it, and a print of
// it leaving out the navigation and the detail panel. Each fixture's counts and lists are held to
// fixtures/README.md by the engine's tests. The server host draws it from the page's own
// derivation of the server's document, which web/wasm's agreement cases hold to the server's
// projection.
import { expect, test } from "@playwright/test";

import { nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

test("C18: the journey card opens the Summary page, a node opens beside it, and a print leaves out navigation and the panel", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "next/list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("journey-card")).toHaveAttribute("data-full", "false");
  await expect(page.getByTestId("card-progress")).toHaveText("2 of 8 in scope done, 6 to go");
  await page.getByTestId("card-expand").click();
  await expect(page).toHaveURL(/\/journeys\/j_bakeoff\/summary$/);
  await expect(page.getByTestId("journey-card")).toHaveAttribute("data-full", "true");
  await page.locator('[data-testid="summary-open"] [data-node="n_winner"] a').click();
  await expect(nodePanel(page, "n_winner")).toBeVisible();
  await expect(page).toHaveURL(/\/journeys\/j_bakeoff\/summary\/nodes\/n_winner$/);
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(page.getByTestId("summary")).toBeVisible();
  // Paper has no dark theme: a dark screen prints its panels in black on white.
  const inked = await page.getByTestId("summary-open").evaluate((part) => {
    const muted = part.querySelector(".muted");
    const figure = document.querySelector("[data-testid=card-progress]");
    return [part, muted, figure].map((each) => (each === null ? "missing" : `${getComputedStyle(each).backgroundColor} ${getComputedStyle(each).color}`));
  });
  expect(inked).toEqual(["rgb(255, 255, 255) rgb(0, 0, 0)", "rgba(0, 0, 0, 0) rgb(105, 105, 105)", "rgba(0, 0, 0, 0) rgb(0, 0, 0)"]);
  await expect(page.getByTestId("journey-toolbar")).toBeHidden();
  await expect(nodePanel(page, "n_winner")).toBeHidden();
  await expect(page.getByRole("button", { name: "Print" })).toBeHidden();
});
