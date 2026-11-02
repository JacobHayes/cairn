// PRD: C20. A phone's screens, for what the walks' phone runs do not reach (they hold every
// state's controls to the touch target, DESIGN, Tokens): Home's first screen, a scrolled menu's
// headings, the timeline's fold under its row's link, chips that wrap, and the inspector's More
// menu. On cairn demo.
import type { Locator } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { nodePanel, open, openAt, whenSettled } from "./shell.ts";

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

test("a phone's first screen, menus, chips and folds take a finger", async ({ page }) => {
  await open(page, "/");
  // The tour's offer is compact: the person's own work starts on the first screen, and a touch screen is not told about keys.
  await expect(page.locator(".tour-keys")).toBeHidden();
  const waiting = await whenSettled(page, () => page.getByRole("heading", { name: "Waiting on you" }).boundingBox());
  expect(waiting?.y ?? Infinity).toBeLessThan(844 - 120);

  await openAt(page, "j_launch", "graph");
  await page.getByTestId("controls-button").click();
  await expectNoOrphanHeading(page.getByTestId("controls-panel-phone"));
  await page.keyboard.press("Escape");

  // The title's link stretches over its row, and the fold button stays tappable above it.
  await openAt(page, "j_launch", "timeline");
  const fold = page.getByTestId("timeline-fold").first();
  const before = await whenSettled(page, () => fold.getAttribute("aria-expanded"));
  await fold.click();
  await expect(fold).not.toHaveAttribute("aria-expanded", before ?? "");

  await openAt(page, "j_launch", "list");
  const wrapped = await whenSettled(page, () => page.locator(".badge").evaluateAll((all) => all.filter((each) => each.getBoundingClientRect().height > 24 || each.scrollWidth > each.clientWidth + 1).map((each) => each.textContent)));
  expect(wrapped, "chips that wrap or are cut").toEqual([]);

  await openAt(page, "j_launch", "list", { node: "n_go_no_go" });
  await expect(nodePanel(page, "n_go_no_go")).toBeVisible();
  await page.getByTestId("more-actions").click();
  await checkpoint(page, "the inspector's menu");
});
