// PRD: C20. A phone's screens: every control a finger taps has a hit area of at least the touch
// target in each direction (DESIGN, Tokens), on Home, the graph with the inspector's sheet, the
// list, the inspector's forms, an open menu and the tour's step. On cairn demo.
import type { Locator } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { reviewOpen } from "./proposals.ts";
import { nodePanel, open, openAt, openFromCanvas, openSheet, whenSettled } from "./shell.ts";

/** A scrolled menu never ends on a section's heading with its options out of view. */
async function expectNoOrphanHeading(menu: Locator): Promise<void> {
  const orphans = await whenSettled(menu.page(), () =>
    menu.evaluate((panel) => {
      const body = panel.closest(".popover-body") ?? panel;
      const edge = body.getBoundingClientRect().bottom;
      return [...panel.querySelectorAll<HTMLElement>(".label, legend")].flatMap((heading) => {
        const top = heading.getBoundingClientRect().top;
        const next = [...panel.querySelectorAll<HTMLElement>("input, button, select")].find((control) => control.getBoundingClientRect().top >= heading.getBoundingClientRect().bottom - 1);
        return top < edge && next !== undefined && next.getBoundingClientRect().top + 22 > edge ? [heading.textContent] : [];
      });
    }),
  );
  expect(orphans, "headings whose options are out of view").toEqual([]);
  // A body that scrolls shows it at its cut edge.
  const cue = await whenSettled(menu.page(), () => menu.evaluate((panel) => {
    const body = panel.closest(".popover-body");
    const style = body === null ? undefined : getComputedStyle(body);
    return { scrolls: body !== null && body.scrollHeight > body.clientHeight, shadow: style?.backgroundImage.includes("gradient") === true && style.backgroundAttachment.includes("scroll") };
  }));
  expect(cue.scrolls ? cue.shadow : true, "a scrolling menu shows its cut edge").toBe(true);
}

test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });

test("every control on a phone's main screens has a touch-sized hit area", async ({ page }) => {
  await open(page, "/");
  await checkpoint(page, "Home");
  // The tour's offer is compact: the person's own work starts on the first screen, and a touch screen is not told about keys.
  await expect(page.locator(".tour-keys")).toBeHidden();
  const waiting = await whenSettled(page, () => page.getByRole("heading", { name: "Waiting on you" }).boundingBox());
  expect(waiting?.y ?? Infinity).toBeLessThan(844 - 120);

  await openAt(page, "j_launch", "graph");
  await checkpoint(page, "the graph");
  await page.getByTestId("controls-button").click();
  await checkpoint(page, "the graph's Controls menu");
  await expectNoOrphanHeading(page.getByTestId("controls-panel-phone"));
  await page.keyboard.press("Escape");
  await page.getByTestId("view-picker").click();
  await checkpoint(page, "the View picker");
  await page.keyboard.press("Escape");

  for (const view of ["summary", "timeline", "walk"]) {
    await openAt(page, "j_launch", view);
    await checkpoint(page, `the ${view} view`);
    if (view === "timeline") {
      // The title's link stretches over its row, and the fold button stays tappable above it.
      const fold = page.getByTestId("timeline-fold").first();
      const before = await whenSettled(page, () => fold.getAttribute("aria-expanded"));
      await fold.click();
      await expect(fold).not.toHaveAttribute("aria-expanded", before ?? "");
    }
  }
  for (const index of ["/journeys", "/routes", "/identities"]) {
    await open(page, index);
    await checkpoint(page, index);
  }

  await openAt(page, "j_launch", "list");
  await checkpoint(page, "the list");
  const wrapped = await whenSettled(page, () => page.locator(".badge").evaluateAll((all) => all.filter((each) => each.getBoundingClientRect().height > 24 || each.scrollWidth > each.clientWidth + 1).map((each) => each.textContent)));
  expect(wrapped, "chips that wrap or are cut").toEqual([]);

  await openAt(page, "j_launch", "list", { node: "n_go_no_go" });
  await expect(nodePanel(page, "n_go_no_go")).toBeVisible();
  await checkpoint(page, "the inspector");
  await page.getByTestId("more-actions").click();
  await checkpoint(page, "the inspector's menu");
  await page.keyboard.press("Escape");

  await openAt(page, "j_launch", "graph", { node: "n_go_no_go" });
  await checkpoint(page, "the graph with the sheet raised");
});

test("every control on the tour's step has a touch-sized hit area", async ({ page }) => {
  await open(page, "/");
  await page.getByTestId("tour-start").click();
  await expect(page.getByTestId("tour-callout")).toHaveAttribute("data-step", "map");
  await checkpoint(page, "the tour's first step");
  const panel = await openFromCanvas(page, "n_outside");
  await page.getByTestId("tour-next").click();
  await expect(page.getByTestId("tour-callout")).toHaveAttribute("data-step", "answer");
  await checkpoint(page, "the tour's answer step, with Back");
  await openSheet(page);
  await expect(panel.getByRole("button", { name: "Save", exact: true })).toBeVisible();
  await checkpoint(page, "the tour's decision in the raised sheet");
});

test("the proposal band's Apply and Close are whole on a phone", async ({ page }) => {
  await open(page, "/journeys/j_vendor_eval?view=graph");
  await page.getByTestId("journey-menu").click();
  await page.getByTestId("menu-save").click();
  await page.getByTestId("save-as-route-flow").getByRole("button", { name: "Propose saving it as a route" }).click();
  await reviewOpen(page);
  // Apply once shrank to the touch-sized minimum width and cut its own label.
});
