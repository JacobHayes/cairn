// The Summary page (C18) in print, which only a browser renders: the navigation and the detail
// panel left out, paper's black on white whatever the screen's theme. Its counts and lists are
// held to fixtures/README.md by the engine's tests and read by summary/model.test.ts.
import { expect, test } from "@playwright/test";

import { nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

test("C18: a print of the Summary page leaves out navigation and the panel, and paints black on white", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", { node: "n_winner", fixedToday: FIXED_TODAY });
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(page.getByTestId("summary")).toBeVisible();
  // Paper has no dark theme: a dark screen prints its panels in black on white, and muted text in
  // the print theme's muted token (resolved through an element, as the page resolves it).
  const { painted, muted } = await page.getByTestId("summary-open").evaluate((part) => {
    const probe = document.body.appendChild(document.createElement("span"));
    probe.style.color = "var(--color-muted)";
    const token = getComputedStyle(probe).color;
    probe.remove();
    const paint = (each: Element | null) => (each === null ? "missing" : `${getComputedStyle(each).backgroundColor} ${getComputedStyle(each).color}`);
    return {
      painted: [part, part.querySelector(".muted"), document.querySelector("[data-testid=card-progress]")].map(paint),
      muted: token,
    };
  });
  expect(painted).toEqual(["rgb(255, 255, 255) rgb(0, 0, 0)", `rgba(0, 0, 0, 0) ${muted}`, "rgba(0, 0, 0, 0) rgb(0, 0, 0)"]);
  await expect(page.getByTestId("journey-toolbar")).toBeHidden();
  await expect(nodePanel(page, "n_winner")).toBeHidden();
  await expect(page.getByRole("button", { name: "Print" })).toBeHidden();
});
