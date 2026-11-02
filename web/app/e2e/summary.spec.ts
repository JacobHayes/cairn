// The Summary page (C18) in Chromium: the journey's front page, its rows acting in place, its
// links, and how it fits a phone, a tablet beside the sheet, paper and an archived journey.
// What each value is (the final milestone, the claimed rows, what finished lately) is the
// engine-backed model's (summary/model.engine.test.ts); this is what only a browser shows. Every
// test runs on cairn demo, whose fixtures are fresh on every load.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { countOf, keysOf, panelOf, rowsOf, stall, startOnSummary } from "./summary.ts";
import { goTo, nodePanel, openAt, whenSettled } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";
import { expectStructure } from "./structure.ts";

const fixed = { fixedToday: FIXED_TODAY };

test("C10, C18, E1: a row's title opens that node on the graph; Done on a row of Waiting on you acts in place, and the lead moves with the derivation", async ({ page }) => {
  await openAt(page, "j_launch", "summary", fixed);
  // A row's title opens that node on the graph, and back is the Summary again.
  const first = (await keysOf(rowsOf(page, "summary-yours")))[0] ?? "";
  await rowsOf(page, "summary-yours").first().locator("a.node-title").click();
  await expect(page).toHaveURL(new RegExp(`/journeys/j_launch/nodes/${first}`));
  await expect(nodePanel(page, first)).toBeVisible();
  await page.goBack();
  await expect(page.getByTestId("summary-lead")).toBeVisible();
  const yours = panelOf(page, "summary-yours");
  const left = page.getByTestId("summary-left");
  const before = Number(await whenSettled(page, () => left.getAttribute("data-remaining")));
  const doneBefore = Number(await whenSettled(page, () => page.locator('[data-testid="summary-states"] [data-state="done"]').getAttribute("data-count")));
  const row = yours.locator('[data-testid="summary-item"][data-node="n_retro"]');
  await row.hover();
  await row.getByRole("button", { name: "Mark done", exact: true }).click();
  await expect(row).toHaveCount(0);
  // What is left is what the derivation now says (finishing a node can finish what it alone held up), so it is less, not "one less".
  await expect.poll(async () => Number(await left.getAttribute("data-remaining"))).toBeLessThan(before);
  await expect.poll(async () => Number(await page.locator('[data-testid="summary-states"] [data-state="done"]').getAttribute("data-count"))).toBeGreaterThan(doneBefore);
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "summary");
  // The focus moved to a row that is still there, not to the page.
  await expect(page.locator("body")).not.toBeFocused();
  await expect(yours.locator(":focus")).toHaveCount(1);
});

test("C11: Decide goes to the node's address and puts the focus in its answer form", async ({ page }) => {
  await startOnSummary(page, "vendor-evaluation");
  const decide = panelOf(page, "summary-decide");
  await expect(decide).toBeVisible();
  const first = (await keysOf(rowsOf(page, "summary-decide")))[0] ?? "";
  const row = decide.locator(`[data-testid="summary-item"][data-node="${first}"]`);
  await row.hover();
  await row.getByRole("button", { name: "Decide" }).click();
  await expect(page).toHaveURL(new RegExp(`/nodes/${first}`));
  await expect(nodePanel(page, first).locator('[data-testid="answer-editor"] :is(select, input, textarea)').first()).toBeFocused();
});

/** On a phone the first action row is in the first screen and nothing runs off the side. */
async function phoneFitsFirstScreen(page: Page): Promise<void> {
  await page.setViewportSize({ width: 390, height: 844 });
  await openAt(page, "j_vendor_eval", "summary", fixed);
  const first = rowsOf(page, "summary-yours").first();
  await expect(first).toBeVisible();
  const box = await whenSettled(page, () => first.boundingBox());
  expect((box?.y ?? 9999) + (box?.height ?? 0)).toBeLessThanOrEqual(844);
  await expectStructure(page, "the summary");
  // Who has it and what to do stay visible and reachable, the owner under the title in its column.
  await expect(first.locator(".row-owner")).toBeVisible();
  const [owner, title] = await whenSettled(page, () => Promise.all([first.locator(".row-owner").boundingBox(), first.locator(".node-title").boundingBox()]));
  expect(Math.abs((owner?.x ?? 0) - (title?.x ?? 99))).toBeLessThanOrEqual(1);
  expect(owner?.y ?? 0).toBeGreaterThan(title?.y ?? 9999);
  await first.locator(".row-act button").first().scrollIntoViewIfNeeded();
  expect(await whenSettled(page, () => first.locator(".row-act button").first().boundingBox())).not.toBeNull();
}

test("C18, D8: on a phone the first action row is in the first screen and nothing runs off the side; beside the sheet at 1024px the Summary is one column and nothing is clipped; work under way and blocked work are listed, each blocked node saying what it waits for", async ({ page }) => {
  await phoneFitsFirstScreen(page);
  await page.setViewportSize({ width: 1024, height: 700 });
  await openAt(page, "j_vendor_eval", "summary", { ...fixed, node: "n_plan" });
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  const columns = await whenSettled(page, () => page.locator(".summary-grid").evaluate((grid) => getComputedStyle(grid).gridTemplateColumns.split(" ").length));
  expect(columns).toBe(1);
  const clipped = await whenSettled(page, () =>
    page.evaluate(() => {
      const scroller = document.querySelector(".ws-body");
      return scroller === null ? 0 : scroller.scrollWidth - scroller.clientWidth;
    }),
  );
  expect(clipped).toBeLessThanOrEqual(0);
  await expect(panelOf(page, "summary-underway")).toBeVisible();
  const blocked = page.locator('details[data-testid="summary-blocked"]');
  await blocked.locator("summary").click();
  const report = rowsOf(page, "summary-blocked").and(page.locator('[data-node="n_final_report"]'));
  await expect(report).toHaveCount(1);
  await expect(report.getByTestId("fact")).toContainText("Final review opens");
});

test("C18, H6: a list past five rows shows five and Show all reveals the rest with their actions; print is the whole dated report: no controls, every row, owners, black on white, a fold the viewer opened open", async ({ page }) => {
  await startOnSummary(page, "product-launch");
  const next = panelOf(page, "summary-next");
  const shown = rowsOf(page, "summary-next");
  await expect.poll(async () => await countOf(page, "summary-next")).toBeGreaterThan(5);
  const total = await countOf(page, "summary-next");
  await expect(shown).toHaveCount(5);
  await next.getByTestId("rows-show-all").click();
  await expect(shown).toHaveCount(total);
  await expect(shown.last().locator(".row-act button").first()).toBeAttached();
  await next.getByTestId("rows-show-all").click();
  await expect(shown).toHaveCount(5);
  // A fold the viewer opened and closed again is open on paper.
  const fold = page.locator("details.rows-fold").first();
  await fold.locator("summary").click();
  await expect(fold).toHaveJSProperty("open", true);
  await fold.locator("summary").click();
  await expect(fold).toHaveJSProperty("open", false);
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(fold).toHaveJSProperty("open", true);
  await expect(page.getByTestId("view-picker")).toBeHidden();
  await expect(page.getByTestId("journey-menu")).toBeHidden();
  await expect(page.getByTestId("rows-show-all").first()).toBeHidden();
  // The list that was collapsed to five prints whole.
  await expect(shown).toHaveCount(total);
  await expect(page.locator(".summary .row-act").first()).toBeHidden();
  // A foot holds only controls, so it is no empty box on paper.
  await expect(page.locator(".summary .node-panel-foot").first()).toBeHidden();
  await expect(page.locator(".summary-asof")).toBeVisible();
  const painted = await whenSettled(page, () => page.getByTestId("summary-lead").evaluate((lead) => `${getComputedStyle(lead).backgroundColor} ${getComputedStyle(lead).color}`));
  expect(painted).toBe("rgb(255, 255, 255) rgb(0, 0, 0)");
});

test("D5: the list's stalled panel offers the same actions, and an unsnooze acts in place", async ({ page }) => {
  await stall(page, "j_bakeoff", ["n_trial_a", "n_trial_b", "n_wrap_up"]);
  await goTo(page, "list");
  const causes = page.getByTestId("stall-cause");
  await expect(causes.first()).toBeVisible();
  // Each cause is one row: its action sits beside the title, not under it.
  const [title, action] = await whenSettled(page, () => Promise.all([causes.first().locator("a").first().boundingBox(), causes.first().getByRole("button").boundingBox()]));
  expect(action?.y ?? 0).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
  const before = await whenSettled(page, () => causes.count());
  await page.getByRole("button", { name: "Unsnooze" }).first().click();
  // The unsnoozed node can be acted on again, so nothing is stalled.
  await expect(page.getByTestId("stalled")).toHaveCount(0);
  expect(before).toBeGreaterThan(0);
});

