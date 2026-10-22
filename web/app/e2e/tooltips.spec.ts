// The one tooltip (ui/Tip.tsx): it shows after a rest and at once between neighbours, on keyboard
// focus, stays for a pointer that moves onto it, hides on Esc without anything else happening,
// stays inside the window, shows a cut title whole and a fitting one not at all, and is
// reachable by tap and by keyboard where a tip is a fact's only place. On the in-browser host.
// The browser's own tooltip is excluded on every page by the invariants (e2e/tips.ts).
import type { Locator, Page } from "@playwright/test";

import { pointOn } from "./canvas.ts";
import { open, openAt, nodeCard, nodePanel, settled, whenSettled } from "./shell.ts";
import { publishStressJourney } from "./stress.ts";
import { expect, test } from "./test.ts";
import { HOVER_LEAVE_MS, TIP_SHOW_MS } from "../src/ui/tip.ts";
import { expectTipInWindow, openTip } from "./tips.ts";

const FIXTURE = "j_launch";

/** Presses Tab until `target` has the focus: a keyboard's own way there, so the focus ring (and the tip that follows it) shows. */
async function tabTo(page: Page, target: Locator): Promise<void> {
  for (let presses = 0; presses < 80; presses += 1) {
    await page.keyboard.press("Tab");
    if (await whenSettled(page, () => target.evaluate((element) => element === document.activeElement))) {
      return;
    }
  }
  throw new Error("Tab never reached the control");
}

/** The words of the tip that is open. */
const tipWords = async (page: Page): Promise<string> => (await whenSettled(page, () => openTip(page).locator(".tip-words").textContent())) ?? "";

test("a control's tip waits for the pointer to rest, then follows it to a neighbour at once, with its key, and stays for a pointer moving onto it", async ({ page }) => {
  await page.clock.install();
  await openAt(page, "browser", FIXTURE, "graph");
  const depth = page.getByTestId("depth-button");
  const frontier = page.getByTestId("chip-frontier");
  await depth.hover();
  await expect(openTip(page)).toHaveCount(0);
  await page.clock.runFor(TIP_SHOW_MS);
  await expect(openTip(page)).toBeVisible();
  await expectTipInWindow(page);
  await expect(depth).toHaveAccessibleDescription(await tipWords(page));
  // Straight onto the neighbour: its tip is there with no wait, and carries its key.
  await frontier.hover();
  await expect(openTip(page).locator("kbd")).toHaveText("Shift F");
  await expect(frontier).toHaveAccessibleDescription(await tipWords(page));
  // Slowly onto the tip itself: it stays, hoverable.
  const [control, tip] = await whenSettled(page, () => Promise.all([frontier.boundingBox(), openTip(page).boundingBox()]));
  if (control === null || tip === null) {
    throw new Error("no boxes");
  }
  const [x, y] = [control.x + control.width / 2, control.y + control.height - 1];
  await page.mouse.move(x, y);
  for (const at of [y + 3, tip.y + 2]) {
    await page.mouse.move(x, at);
    await page.clock.runFor(HOVER_LEAVE_MS / 4);
  }
  await page.clock.runFor(3 * HOVER_LEAVE_MS);
  await expect(openTip(page)).toBeVisible();
  // Off both: gone.
  await page.mouse.move(5, 400);
  await page.clock.runFor(3 * HOVER_LEAVE_MS);
  await expect(openTip(page)).toHaveCount(0);
});

test("a tip shows at once on keyboard focus, stays while the focus does, and Esc hides it and nothing else", async ({ page }) => {
  await page.clock.install();
  await openAt(page, "browser", FIXTURE, "graph", { node: "n_announcement" });
  await expect(nodePanel(page, "n_announcement")).toBeVisible();
  const frontier = page.getByTestId("chip-frontier");
  await tabTo(page, frontier);
  expect(await tipWords(page)).toContain("Light up");
  await page.mouse.move(300, 500);
  await page.clock.runFor(2 * TIP_SHOW_MS);
  await expect(openTip(page)).toBeVisible();
  // The first Esc hides the tip: the focus and the selection stay. The second does the usual.
  await page.keyboard.press("Escape");
  await expect(openTip(page)).toHaveCount(0);
  await expect(frontier).toBeFocused();
  await expect(nodePanel(page, "n_announcement")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, "n_announcement")).toHaveCount(0);
});

test("the inspector's Hide tip keeps clear of the window's right edge", async ({ page }) => {
  await openAt(page, "browser", FIXTURE, "graph", { node: "n_announcement" });
  await page.getByRole("button", { name: "Hide inspector" }).hover();
  await expect(openTip(page)).toBeVisible();
  await expectTipInWindow(page, 8);
  await expect(openTip(page).locator("kbd")).toHaveText("]");
});

test("a card title cut short shows whole in a tip, promptly; a title that fits shows none; a mark on a card shows its own", async ({ page }) => {
  await page.clock.install();
  await page.setViewportSize({ width: 4200, height: 3200 });
  const journey = await publishStressJourney(page);
  await openAt(page, "browser", journey, "graph");
  await page.keyboard.press("f");
  await settled(page);
  const opens = page.getByTestId("card-open");
  const cut = await whenSettled(page, () => opens.evaluateAll((all) => all.flatMap((each, at) => (each.scrollHeight > each.clientHeight + 1 || [...each.querySelectorAll("*")].some((inner) => inner.scrollHeight > inner.clientHeight + 1) ? [at] : []))));
  const fits = await whenSettled(page, () => opens.evaluateAll((all) => all.flatMap((each, at) => (each.scrollHeight <= each.clientHeight + 1 && ![...each.querySelectorAll("*")].some((inner) => inner.scrollHeight > inner.clientHeight + 1) ? [at] : []))));
  expect(cut.length, "the stress journey cuts some card titles").toBeGreaterThan(0);
  expect(fits.length, "the stress journey has some card titles that fit").toBeGreaterThan(0);
  const long = opens.nth(cut[0] ?? 0);
  const whole = (await whenSettled(page, () => long.textContent()))?.trim() ?? "";
  await long.hover();
  // Promptly: the moment the show delay is up, with the whole title.
  await page.clock.runFor(TIP_SHOW_MS);
  await expect(openTip(page)).toHaveText(whole);
  await expectTipInWindow(page);
  await page.mouse.move(5, 5);
  await expect(openTip(page)).toHaveCount(0);
  await opens.nth(fits[0] ?? 0).hover();
  await page.clock.runFor(2 * TIP_SHOW_MS);
  await expect(openTip(page)).toHaveCount(0);
  await nodeCard(page, "n_stage_a").waitFor();
});

test("a graph line shows its words at once when the keyboard focuses it, and they are its accessible name", async ({ page }) => {
  await openAt(page, "browser", FIXTURE, "graph");
  await page.keyboard.press("Tab");
  const line = page.locator(".react-flow__edge:not(.selectable)").or(page.locator(".react-flow__edge")).first();
  await line.focus();
  await expect(openTip(page)).toBeVisible();
  await expect(line).toHaveAttribute("aria-label", await tipWords(page));
});

test.describe("on a phone", () => {
  test.use({ hasTouch: true, viewport: { width: 390, height: 844 } });

  test("a tap on a fact shows it and a tap elsewhere hides it; the same fact is reached by Tab and Enter", async ({ page }) => {
    await openAt(page, "browser", FIXTURE, "summary");
    const date = page.locator("[data-tip-tap]").first();
    await date.tap();
    await expect(openTip(page)).toBeVisible();
    await expectTipInWindow(page);
    // Above the fact: the finger is not over it and the line below stays readable.
    const [tip, fact] = await whenSettled(page, () => Promise.all([openTip(page).boundingBox(), date.boundingBox()]));
    expect(tip?.y ?? Infinity).toBeLessThan(fact?.y ?? 0);
    await page.locator("h1").first().tap();
    await expect(openTip(page)).toHaveCount(0);
    await date.focus();
    await page.keyboard.press("Enter");
    await expect(openTip(page)).toBeVisible();
  });
});

test("a held control takes the focus, says why, and a press does nothing", async ({ page }) => {
  await openAt(page, "browser", FIXTURE, "walk");
  const frontier = page.getByTestId("chip-frontier");
  await expect(frontier).toHaveAttribute("aria-disabled", "true");
  const before = page.url();
  await tabTo(page, frontier);
  await expect(openTip(page)).toBeVisible();
  await expect(frontier).toHaveAccessibleDescription(await tipWords(page));
  await page.keyboard.press("Enter");
  await page.keyboard.press("Space");
  await frontier.click({ force: true });
  await expect(frontier).toHaveAttribute("aria-pressed", "false");
  expect(page.url()).toBe(before);
});

test("the rail's ? is drawn where the page binds keys, previews them on hover, and opens the key sheet", async ({ page }) => {
  const keys = page.getByRole("button", { name: "Keyboard shortcuts" });
  await open(page, "browser", "/");
  await expect(keys).toHaveCount(0);
  await openAt(page, "browser", FIXTURE, "graph");
  await expect(keys).toBeVisible();
  await keys.hover();
  await expect(openTip(page)).toBeVisible();
  await expect(openTip(page)).toContainText("Cycle Graph, List and Timeline");
  await expectTipInWindow(page, 12);
  await keys.click();
  await expect(page.getByTestId("key-sheet")).toBeVisible();
});

test("a line's tip goes when the view that drew it does, without the pointer moving", async ({ page }) => {
  await openAt(page, "browser", FIXTURE, "graph");
  await settled(page);
  const at = await pointOn(page.locator(".react-flow__edge").first());
  await page.mouse.move(at.x, at.y);
  await expect(openTip(page)).toBeVisible();
  await page.keyboard.press("v");
  await expect(page.getByTestId("canvas")).toHaveCount(0);
  await expect(openTip(page)).toHaveCount(0);
});

test("a timeline title cut short shows whole in a tip that is not itself cut", async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const journey = await publishStressJourney(page);
  await openAt(page, "browser", journey, "timeline");
  const links = page.locator(".timeline-text a");
  // A title that is cut and already in the window: the pointer goes straight to it, and nothing has to scroll (a scroll under the pointer closes a tip, as it should).
  const cut = await whenSettled(page, () => links.evaluateAll((all) => all.flatMap((each, at) => (each.scrollWidth > each.clientWidth + 1 && each.getBoundingClientRect().top >= 0 && each.getBoundingClientRect().bottom <= window.innerHeight ? [at] : []))));
  expect(cut.length, "the stress journey cuts some timeline titles inside the window").toBeGreaterThan(0);
  const long = links.nth(cut[0] ?? 0);
  const whole = (await whenSettled(page, () => long.textContent()))?.trim() ?? "";
  await long.hover();
  await expect(openTip(page)).toHaveText(whole);
  await expect.poll(() => openTip(page).evaluate((tip) => tip.scrollWidth - tip.clientWidth)).toBeLessThanOrEqual(1);
});

test("a scroll in another scroller does not take a tip from a pointer resting on its control", async ({ page }) => {
  await page.clock.install();
  await openAt(page, "browser", FIXTURE, "graph");
  await page.getByTestId("chip-frontier").hover();
  await expect(openTip(page)).toHaveCount(0);
  await page.evaluate(() => {
    const other = document.createElement("div");
    other.style.cssText = "position:fixed;left:0;bottom:0;width:20px;height:20px;overflow:auto";
    other.innerHTML = '<div style="height:200px"></div>';
    document.body.append(other);
    other.scrollTop = 40;
  });
  await page.clock.runFor(TIP_SHOW_MS);
  await expect(openTip(page)).toBeVisible();
});
