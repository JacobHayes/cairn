// The Summary page (C18) in Chromium: the journey's front page, its rows acting in place, its
// links, and how it fits a phone, a tablet beside the sheet, paper and an archived journey.
// What each value is (the final milestone, the claimed rows, what finished lately) is the
// engine-backed model's (summary/model.engine.test.ts); this is what only a browser shows. Every
// test runs on the in-browser host, whose fixtures are fresh on every load.
import { expect, test } from "./test.ts";

import { setStatus } from "./around.ts";
import { countOf, keysOf, panelOf, rowsOf, stall, startOnSummary } from "./summary.ts";
import { goTo, nodePanel, openAt } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

const fixed = { fixedToday: FIXED_TODAY };

test("C18: the bare Summary leads with what is left, and a row's title opens that node on the graph", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", fixed);
  const lead = page.getByTestId("summary-lead");
  await expect(lead).toBeVisible();
  const left = page.getByTestId("summary-left");
  const states = await page.getByTestId("summary-states").locator("[data-count]").evaluateAll((each) => each.map((one) => Number(one.getAttribute("data-count"))));
  // The counts by state are the whole scope, and what is left is some of it.
  expect(states.reduce((sum, each) => sum + each, 0)).toBe(Number(await left.getAttribute("data-in-scope")));
  expect(Number(await left.getAttribute("data-remaining"))).toBeLessThanOrEqual(Number(await left.getAttribute("data-in-scope")));
  await expect(page.getByTestId("summary-milestone")).toHaveAttribute("data-role", "final");
  await expect(rowsOf(page, "summary-next").first()).toBeVisible();
  const first = (await keysOf(rowsOf(page, "summary-next")))[0] ?? "";
  await rowsOf(page, "summary-next").first().locator("a.node-title").click();
  await expect(page).toHaveURL(new RegExp(`/journeys/j_bakeoff/nodes/${first}`));
  await expect(nodePanel(page, first)).toBeVisible();
  await page.goBack();
  await expect(page.getByTestId("summary-lead")).toBeVisible();
});

test("C10, E1: Done on a row of Waiting on you acts in place, and the lead moves with the derivation", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "summary", fixed);
  const yours = panelOf(page, "summary-yours");
  const left = page.getByTestId("summary-left");
  const before = Number(await left.getAttribute("data-remaining"));
  const doneBefore = Number(await page.locator('[data-testid="summary-states"] [data-state="done"]').getAttribute("data-count"));
  const row = yours.locator('[data-testid="summary-item"][data-node="n_announcement"]');
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

test("C18: a list past five rows shows five, and Show all reveals the rest with their actions", async ({ page }) => {
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
});

test("C18: on a phone the first action row is in the first screen and nothing runs off the side", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openAt(page, "browser", "j_vendor_eval", "summary", fixed);
  const first = rowsOf(page, "summary-yours").first();
  await expect(first).toBeVisible();
  const box = await first.boundingBox();
  expect((box?.y ?? 9999) + (box?.height ?? 0)).toBeLessThanOrEqual(844);
  const beyond = await page.evaluate(() => [...document.querySelectorAll(".summary *")].filter((each) => each.getBoundingClientRect().right > innerWidth + 1).map((each) => each.getAttribute("class") ?? ""));
  expect(beyond).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0);
  // Who has it and what to do stay visible and reachable, the owner under the title in its column.
  await expect(first.locator(".row-owner")).toBeVisible();
  const [owner, title] = await Promise.all([first.locator(".row-owner").boundingBox(), first.locator(".node-title").boundingBox()]);
  expect(Math.abs((owner?.x ?? 0) - (title?.x ?? 99))).toBeLessThanOrEqual(1);
  expect(owner?.y ?? 0).toBeGreaterThan(title?.y ?? 9999);
  await first.locator(".row-act button").first().scrollIntoViewIfNeeded();
  expect(await first.locator(".row-act button").first().boundingBox()).not.toBeNull();
});

test("C18: beside the sheet at 1024px the Summary is one column and nothing is clipped", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  await openAt(page, "browser", "j_vendor_eval", "summary", { ...fixed, node: "n_plan" });
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  const columns = await page.locator(".summary-grid").evaluate((grid) => getComputedStyle(grid).gridTemplateColumns.split(" ").length);
  expect(columns).toBe(1);
  const clipped = await page.evaluate(() => {
    const scroller = document.querySelector(".ws-body");
    return scroller === null ? 0 : scroller.scrollWidth - scroller.clientWidth;
  });
  expect(clipped).toBeLessThanOrEqual(0);
});

test("C18, H6: print is the whole dated report: no controls, every row, owners, black on white", async ({ page }) => {
  await startOnSummary(page, "product-launch");
  await page.emulateMedia({ media: "print", colorScheme: "dark" });
  await expect(page.getByTestId("view-picker")).toBeHidden();
  await expect(page.getByTestId("journey-menu")).toBeHidden();
  await expect(page.getByTestId("rows-show-all").first()).toBeHidden();
  await expect.poll(async () => await countOf(page, "summary-next")).toBeGreaterThan(0);
  const total = await countOf(page, "summary-next");
  await expect(rowsOf(page, "summary-next")).toHaveCount(total);
  await expect(page.locator(".summary .row-act").first()).toBeHidden();
  // A foot holds only controls, so it is no empty box on paper.
  await expect(page.locator(".summary .node-panel-foot").first()).toBeHidden();
  await expect(page.locator(".summary-asof")).toBeVisible();
  const painted = await page.getByTestId("summary-lead").evaluate((lead) => `${getComputedStyle(lead).backgroundColor} ${getComputedStyle(lead).color}`);
  expect(painted).toBe("rgb(255, 255, 255) rgb(0, 0, 0)");
});

test("C18, A19: an archived journey's Summary is read-only: its notes are shown and nothing offers a change", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", fixed);
  const about = page.getByTestId("summary-about");
  await about.getByRole("button", { name: "Add a note or link" }).click();
  await about.getByLabel("Note").fill("A note kept on the journey");
  await about.getByRole("button", { name: "Add", exact: true }).click();
  await expect(about.getByTestId("annotation")).toHaveCount(1);
  await setStatus(page, "archived");
  await expect(page.getByTestId("summary-lead")).toBeVisible();
  await expect(about.getByTestId("annotation")).toHaveCount(1);
  await expect(page.locator(".summary .row-act")).toHaveCount(0);
  await expect(about.getByRole("button")).toHaveCount(0);
  await expect(page.getByTestId("journey-upgrade")).toHaveCount(0);
});

test("C18, A19: a stalled archived journey lists what it waits on without the actions", async ({ page }) => {
  await stall(page, "j_bakeoff", ["n_trial_a", "n_trial_b", "n_wrap_up"]);
  await expect(page.getByRole("button", { name: "Unsnooze" }).first()).toBeVisible();
  await setStatus(page, "archived");
  await expect(page.getByTestId("stalled")).toBeVisible();
  await expect(page.getByRole("button", { name: "Unsnooze" })).toHaveCount(0);
});

test("D5: the graph's stalled panel offers the same actions, and an unsnooze acts in place", async ({ page }) => {
  await stall(page, "j_bakeoff", ["n_trial_a", "n_trial_b", "n_wrap_up"]);
  await goTo(page, "graph");
  const causes = page.getByTestId("stall-cause");
  await expect(causes.first()).toBeVisible();
  // Each cause is one row: its action sits beside the title, not under it.
  const [title, action] = await Promise.all([causes.first().locator("a").first().boundingBox(), causes.first().getByRole("button").boundingBox()]);
  expect(action?.y ?? 0).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
  const before = await causes.count();
  await page.getByRole("button", { name: "Unsnooze" }).first().click();
  // The unsnoozed node can be acted on again, so nothing is stalled.
  await expect(page.getByTestId("stalled")).toHaveCount(0);
  expect(before).toBeGreaterThan(0);
});

test("C18: a journey with no node invites planning and offers no completion", async ({ page }) => {
  await startOnSummary(page, undefined);
  await expect(page.getByTestId("summary-empty")).toBeVisible();
  await expect(page.getByTestId("completion-suggested")).toHaveCount(0);
  // The invitation leads and the description sits beside it, not as wide as it.
  const [invite, about] = await Promise.all([page.getByTestId("summary-empty").boundingBox(), page.getByTestId("summary-about").boundingBox()]);
  expect(about?.width ?? 9999).toBeLessThan(invite?.width ?? 0);
  await page.getByTestId("summary-plan").click();
  await expect(page).toHaveURL(/view=graph.*edit=on|edit=on.*view=graph/);
});

test("B11, C18: a completed journey still says what is left, and has no Waiting on you", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "summary", fixed);
  await expect(panelOf(page, "summary-yours")).toBeVisible();
  await setStatus(page, "completed");
  await expect(page.getByTestId("summary-left")).toHaveAttribute("data-remaining", /^[1-9]/);
  await expect(panelOf(page, "summary-yours")).toHaveCount(0);
  // It says it is closed and what it left, and lists that as left unfinished, not as next.
  const remaining = await page.getByTestId("summary-left").getAttribute("data-remaining");
  await expect(page.getByTestId("summary-lifecycle")).toContainText(String(remaining));
  await expect(panelOf(page, "summary-next")).toHaveAttribute("aria-label", /ready or active/i);
  await goTo(page, "list");
  await goTo(page, "summary");
  await expect(page.getByTestId("summary-lead")).toBeVisible();
});

test("B11, C18: a completed journey prints its status beside its name, and not the controls", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", fixed);
  await setStatus(page, "completed");
  await page.emulateMedia({ media: "print" });
  await expect(page.getByTestId("journey-status")).toBeVisible();
  await expect(page.getByTestId("view-picker")).toBeHidden();
});

test("C18, H6: a fold the viewer opened and closed again is open on paper", async ({ page }) => {
  await openAt(page, "browser", "j_bakeoff", "summary", fixed);
  const fold = page.locator("details.rows-fold").first();
  await fold.locator("summary").click();
  await expect(fold).toHaveJSProperty("open", true);
  await fold.locator("summary").click();
  await expect(fold).toHaveJSProperty("open", false);
  await page.emulateMedia({ media: "print" });
  await expect(fold).toHaveJSProperty("open", true);
});

test("D8, C18: work under way and blocked work are listed, each blocked node saying what it waits for", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "summary", fixed);
  await expect(panelOf(page, "summary-underway")).toBeVisible();
  const blocked = page.locator('details[data-testid="summary-blocked"]');
  await blocked.locator("summary").click();
  const report = rowsOf(page, "summary-blocked").and(page.locator('[data-node="n_final_report"]'));
  await expect(report).toHaveCount(1);
  await expect(report.getByTestId("fact")).toContainText("Final review opens");
});
