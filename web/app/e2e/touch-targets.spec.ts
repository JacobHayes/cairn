// PRD: C20. A phone's screens: every control a finger taps has a hit area of at least the touch
// target in each direction (DESIGN, Sizing), on Home, the graph with the inspector's sheet, the
// list, the inspector's forms, an open menu and the tour's step. In-browser host, fresh on every load.
import { expect, test } from "./test.ts";

import { nodePanel, open, openAt, openFromCanvas, openSheet, whenSettled } from "./shell.ts";
import { expectTouchTargets } from "./touch.ts";

test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });

test("every control on a phone's main screens has a touch-sized hit area", async ({ page }) => {
  await open(page, "browser", "/");
  await expectTouchTargets(page, "Home");

  await openAt(page, "browser", "j_launch", "graph");
  await expectTouchTargets(page, "the graph");
  await page.getByTestId("controls-button").click();
  await expectTouchTargets(page, "the graph's Controls menu");
  await page.keyboard.press("Escape");
  await page.getByTestId("view-picker").click();
  await expectTouchTargets(page, "the View picker");
  await page.keyboard.press("Escape");

  for (const view of ["summary", "timeline", "walk"]) {
    await openAt(page, "browser", "j_launch", view);
    await expectTouchTargets(page, `the ${view} view`);
    if (view === "timeline") {
      // The title's link stretches over its row, and the fold button stays tappable above it.
      const fold = page.getByTestId("timeline-fold").first();
      const before = await whenSettled(page, () => fold.getAttribute("aria-expanded"));
      await fold.click();
      await expect(fold).not.toHaveAttribute("aria-expanded", before ?? "");
    }
  }
  for (const index of ["/journeys", "/routes", "/identities"]) {
    await open(page, "browser", index);
    await expectTouchTargets(page, index);
  }

  await openAt(page, "browser", "j_launch", "list");
  await expectTouchTargets(page, "the list");

  await openAt(page, "browser", "j_launch", "list", { node: "n_go_no_go" });
  await expectTouchTargets(page, "the inspector's peek");
  await openSheet(page);
  await expect(nodePanel(page, "n_go_no_go")).toBeVisible();
  await expectTouchTargets(page, "the inspector");
  await page.getByTestId("more-actions").click();
  await expectTouchTargets(page, "the inspector's menu");
  await page.keyboard.press("Escape");

  await openAt(page, "browser", "j_launch", "graph", { node: "n_go_no_go" });
  await expectTouchTargets(page, "the graph with the sheet at its peek");
  await openSheet(page);
  await expectTouchTargets(page, "the graph with the sheet raised");
});

test("every control on the tour's step has a touch-sized hit area", async ({ page }) => {
  await open(page, "browser", "/");
  await page.getByTestId("tour-start").click();
  await expect(page.getByTestId("tour-callout")).toHaveAttribute("data-step", "map");
  await expectTouchTargets(page, "the tour's first step");
  const panel = await openFromCanvas(page, "n_outside");
  await page.getByTestId("tour-next").click();
  await expect(page.getByTestId("tour-callout")).toHaveAttribute("data-step", "answer");
  await expectTouchTargets(page, "the tour's answer step, with Back");
  await openSheet(page);
  await expect(panel.getByRole("button", { name: "Save", exact: true })).toBeVisible();
  await expectTouchTargets(page, "the tour's decision in the raised sheet");
});
