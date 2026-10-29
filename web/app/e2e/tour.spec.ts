// PRD: C20. A guided first run on cairn demo: the welcome opens on Home and never over a
// deep link; a practice journey is made on Start, walked through the ordinary screens under a
// spotlight, and deleted when the tour ends. What each step says is composed from the state the
// engine derived (steps.test.ts); here the oracle is the screen: where the hole and the callout
// are, what stays dim and what lifts after the action, where the person is.
import type { BrowserContext, Locator, Page } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { setFrontier } from "./acting.ts";
import { goWithin, nodeCard, nodePanel, open, openFromCanvas, visit, whenSettled } from "./shell.ts";

const callout = (page: Page) => page.getByTestId("tour-callout");
const ring = (page: Page) => page.getByTestId("tour-ring");

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A locator's box once it has stopped moving (the canvas pans, the page scrolls). */
async function settledBox(target: Locator): Promise<Box> {
  const seen: { box: Box | null } = { box: null };
  await expect
    .poll(async () => {
      const [before, now] = [seen.box, await target.boundingBox()];
      seen.box = now;
      return before !== null && now !== null && Math.abs(before.x - now.x) < 0.5 && Math.abs(before.y - now.y) < 0.5 && Math.abs(before.width - now.width) < 0.5 && Math.abs(before.height - now.height) < 0.5;
    })
    .toBe(true);
  if (seen.box === null) {
    throw new Error("no box");
  }
  return seen.box;
}

const meets = (a: Box, b: Box): boolean => a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
const holds = (outer: Box, inner: Box, slack = 1): boolean => inner.x >= outer.x - slack && inner.y >= outer.y - slack && inner.x + inner.width <= outer.x + outer.width + slack && inner.y + inner.height <= outer.y + outer.height + slack;

/** Starts the tour from Home and returns the practice journey's id. */
async function start(page: Page): Promise<string> {
  await open(page, "/");
  await page.getByTestId("tour-start").click();
  await expect(callout(page)).toHaveAttribute("data-step", "map");
  return /\/journeys\/(j_[^/?]+)/.exec(page.url())?.[1] ?? "";
}

/** Chooses `option` for the practice decision in its open inspector and saves. */
async function decide(page: Page, option: "yes" | "no"): Promise<void> {
  const panel = nodePanel(page, "n_outside");
  await panel.getByRole("radio", { name: new RegExp(`^${option}\\b`, "i") }).check();
  await panel.getByRole("button", { name: "Save", exact: true }).click();
}

/** Goes on once Next is no longer held. */
async function next(page: Page): Promise<void> {
  const button = page.getByTestId("tour-next");
  await expect(button).not.toHaveAttribute("aria-disabled", "true");
  await button.click();
}

const markDraftDone = (page: Page) => page.locator('tr[data-node="n_draft"]').getByRole("button", { name: /done/i }).click();

/** The tour is at `step`, showing its spotlight (the screen dimmed, Next held) or its reveal (the dimming lifted). */
async function expectAt(page: Page, step: string, phase: "spotlight" | "reveal"): Promise<void> {
  await expect(callout(page)).toHaveAttribute("data-step", step);
  await expect(callout(page)).toHaveAttribute("data-phase", phase);
  await expect(page.getByTestId("tour-scrim")).toHaveAttribute("data-lifted", String(phase === "reveal"));
  await expect(callout(page)).toHaveAttribute("data-done", String(phase === "reveal" || step === "dates"));
}

/** The callout is clear of the hole, and inside the window. */
async function expectClear(page: Page, within: { width: number; height: number }): Promise<void> {
  await expect.poll(async () => meets(await settledBox(callout(page)), await settledBox(ring(page)))).toBe(false);
  const box = await settledBox(callout(page));
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.y).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(within.width);
  expect(box.y + box.height).toBeLessThanOrEqual(within.height);
}

/** What the dimming does: a press and the app's keys do nothing, Escape never ends the tour, and the hole follows a pan of the canvas. */
async function exerciseDim(page: Page): Promise<void> {
  // A press on the dimmed page does nothing, and neither do the app's keys; Escape never ends the tour.
  const chip = await settledBox(page.getByTestId("chip-frontier"));
  await page.mouse.click(chip.x + chip.width / 2, chip.y + chip.height / 2);
  await page.keyboard.press("f");
  await page.keyboard.press("?");
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("dialog[open]")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(callout(page)).toBeVisible();
  // The hole follows what it holds when the canvas pans.
  const card = nodeCard(page, "n_outside");
  const before = await settledBox(card);
  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(before.x + before.width / 2 - 60, before.y + before.height / 2 + 30, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(card), 8)).toBe(true);
}

test("the welcome opens on Home and never over a deep link", async ({ page }) => {
  await visit(page, "/journeys");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  // The rail offers it quietly; the link opens the welcome on Home.
  await page.getByTestId("rail-tour").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
  await checkpoint(page);
  // Not now ends the offers; You still offers the tour.
  await page.getByTestId("tour-not-now").click();
  await page.reload();
  await expect(page.getByTestId("rail-tour")).toHaveCount(0);
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  await page.getByRole("link", { name: "You", exact: true }).click();
  await page.getByTestId("you-tour").click();
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
});

/** A second tab for the same user finds the run waiting, not an offer to start another. */
async function sharedWithAnotherTab(context: BrowserContext): Promise<void> {
  const other = await context.newPage();
  await open(other, "/");
  await expect(other.getByTestId("tour-continue")).toBeVisible();
  await expect(other.getByTestId("tour-welcome")).toHaveCount(0);
  await other.close();
}

/** The assistant step: its example reply, which fits whole inside the hole (the reply grows the pane, and the hole with it), and Finish. */
async function showAssistantExample(page: Page): Promise<void> {
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await page.getByTestId("tab-assistant").click();
  const example = page.getByTestId("tour-assistant-example");
  await example.getByTestId("tour-assistant-show").click();
  const reply = example.getByTestId("assistant-reply");
  await expect(reply).toBeVisible();
  // The reply grows the example's pane, and the hole grows with it: the whole reply is inside it.
  await expect(callout(page)).toHaveAttribute("data-phase", "reveal");
  await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(reply))).toBe(true);
  await expect(page.getByTestId("tour-next")).toHaveText("Finish the tour");
}

/** The addresses of the assistant requests the page makes, as they come. */
function assistantRequests(page: Page): string[] {
  const seen: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith("/assistant")) {
      seen.push(request.url());
    }
  });
  return seen;
}

/** Step 6, the assistant shown as an example, then the end of the tour, closed. */
async function endWithTheAssistant(page: Page): Promise<void> {
  await expectAt(page, "assistant", "spotlight");
  await showAssistantExample(page);
  await page.getByTestId("tour-next").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-end")).toBeVisible();
  await checkpoint(page);
  await page.getByTestId("tour-close").click();
  await expect(page.getByTestId("tour-end")).toHaveCount(0);
}

/** The practice journey `id` is deleted, the journeys are as many as before, and no offer comes back. */
async function practiceIsGone(page: Page, id: string, journeyCount: number): Promise<void> {
  await open(page, "/journeys?status=any");
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toHaveCount(0);
  await expect(page.getByTestId("journey-row")).toHaveCount(journeyCount);
  await open(page, "/");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  await expect(page.getByTestId("tour-continue")).toHaveCount(0);
}

test("the tour walks a practice journey, and the journey is gone at the end", async ({ page, context }) => {
  const assistant = assistantRequests(page);
  await open(page, "/journeys?status=any");
  const journeyCount = await whenSettled(page, () => page.getByTestId("journey-row").count());
  const id = await start(page);
  await expect(page.getByTestId("tour-label")).toContainText("of 6");
  await sharedWithAnotherTab(context);
  const window = { width: 1280, height: 720 };

  // Step 1: dimmed but for the decision; Next waits for it to be selected.
  await expectAt(page, "map", "spotlight");
  await expectClear(page, window);
  await checkpoint(page);
  await exerciseDim(page);
  await openFromCanvas(page, "n_outside");
  await expectAt(page, "map", "reveal");
  await next(page);

  // Step 2: the hole holds the whole editor, so the person can choose and save inside it.
  await expectAt(page, "answer", "spotlight");
  const editor = nodePanel(page, "n_outside").getByTestId("answer-editor");
  expect(holds(await settledBox(ring(page)), await settledBox(editor), 6)).toBe(true);
  await decide(page, "yes");
  await expectAt(page, "answer", "reveal");
  expect(meets(await settledBox(callout(page)), await settledBox(nodeCard(page, "n_review")))).toBe(false);
  await checkpoint(page);
  await next(page);

  // Step 3: Frontier.
  await expectAt(page, "frontier", "spotlight");
  await setFrontier(page, true);
  await expectAt(page, "frontier", "reveal");
  await next(page);

  // Step 4: finish the draft in the list; the work that waited for it is ready and clear of the callout.
  await expectAt(page, "finish", "spotlight");
  await expect(page).toHaveURL(/view=list/);
  await markDraftDone(page);
  await expectAt(page, "finish", "reveal");
  const box = await settledBox(callout(page));
  for (const node of ["n_comments", "n_review"]) {
    expect(meets(box, await settledBox(page.locator(`tr[data-node="${node}"]`)))).toBe(false);
  }
  await checkpoint(page);
  // Going back keeps what was done; each earlier step arrives showing what is true now.
  await page.getByTestId("tour-back").click();
  await expectAt(page, "frontier", "reveal");
  await page.getByTestId("tour-back").click();
  await expectAt(page, "answer", "reveal");
  await next(page);
  await next(page);
  await next(page);

  // Step 5: dates, with the section the sentence is about open inside the hole.
  await expectAt(page, "dates", "spotlight");
  await expect(page).toHaveURL(/view=timeline/);
  await expect(page.getByTestId("dates")).toBeVisible();
  await checkpoint(page);
  await page.getByTestId("tour-next").click();
  await endWithTheAssistant(page);
  await practiceIsGone(page, id, journeyCount);
  // The assistant step is an example: nothing was asked of the journey assistant.
  expect(assistant).toEqual([]);
});

test("Tab stays in the callout and the hole, and a card panned out of reach is brought back", async ({ page }) => {
  await start(page);
  await expect(ring(page)).toBeVisible();
  for (let press = 0; press < 8; press += 1) {
    await page.keyboard.press("Tab");
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest('[data-testid="tour-callout"], [data-id="n_outside"]') !== null)).toBe(true);
  }
  const card = nodeCard(page, "n_outside");
  // Drag the card by its visible edge to the window's edge until it has all but left.
  for (let round = 0; round < 6; round += 1) {
    const at = await settledBox(card);
    if (at.x > 1200) {
      break;
    }
    await page.mouse.move(at.x + 5, at.y + at.height / 2);
    await page.mouse.down();
    await page.mouse.move(1279, at.y + at.height / 2, { steps: 6 });
    await page.mouse.up();
  }
  await page.getByTestId("tour-show").click();
  await expect.poll(async () => (await settledBox(card)).x + (await settledBox(card)).width).toBeLessThan(1280);
  await expect(page.getByTestId("tour-show")).toHaveCount(0);
});

test("a menu over the tour has Escape, and Skip deletes the practice journey", async ({ page }) => {
  const id = await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  const menu = page.locator('[data-popover]:not([data-testid="tour-callout"])');
  await nodePanel(page, "n_outside").getByTestId("more-actions").click();
  await expect(menu).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expectAt(page, "answer", "spotlight");
  await page.getByTestId("tour-skip").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(callout(page)).toHaveCount(0);
  await expect(page.getByTestId("tour-end")).toHaveCount(0);
  await open(page, "/journeys?status=any");
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toHaveCount(0);
});

test("leaving the practice journey pauses the tour, and Continue returns to the step", async ({ page }) => {
  const id = await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  // The dimmed rail takes no press, so the person leaves the way the app's own links do.
  await goWithin(page, "/journeys");
  await expect(page.getByTestId("tour-paused")).toBeVisible();
  await expect(callout(page)).toHaveCount(0);
  await page.getByRole("link", { name: "Home", exact: true }).click();
  await expect(page.getByTestId("tour-continue")).toBeVisible();
  // Escape on the card puts it away for now; it does not end the tour, and Home brings it back.
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("tour-continue")).toHaveCount(0);
  await page.getByRole("link", { name: "Journeys", exact: true }).click();
  await page.getByRole("link", { name: "Home", exact: true }).click();
  await expect(page.getByTestId("tour-continue")).toBeVisible();
  await page.getByTestId("tour-resume").click();
  await expect(page).toHaveURL(new RegExp(`/journeys/${id}`));
  await expect(callout(page)).toHaveAttribute("data-step", "answer");
});

test.describe("on a phone, where the inspector is a sheet", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the callout is docked clear of the hole, the sheet rises for the answer, and Frontier is reached through Controls", async ({ page }) => {
    await start(page);
    const phone = { width: 390, height: 844 };
    await expectClear(page, phone);
    await checkpoint(page);
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await expectClear(page, phone);
    // The sheet lowered, the callout says so and offers to raise it.
    await page.getByRole("button", { name: "Resize the inspector" }).focus();
    await page.keyboard.press("ArrowDown");
    await page.getByTestId("tour-show").click();
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await decide(page, "yes");
    // The pane is inert at the peek: the sheet lowers and the reveal is on the screen.
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "peek");
    await expectAt(page, "answer", "reveal");
    await next(page);
    // The hole holds Controls and, once opened, the menu with Frontier in it.
    await expectClear(page, phone);
    await page.getByTestId("controls-button").click();
    const menu = page.getByTestId("controls-button-menu");
    await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(menu), 8)).toBe(true);
    await page.keyboard.press("Escape");
    await setFrontier(page, true);
    await expectAt(page, "frontier", "reveal");
    await next(page);
    await markDraftDone(page);
    await expectAt(page, "finish", "reveal");
    await expect(page.getByTestId("tour-next")).toBeVisible();
  });
});

test.describe("under 1100px", () => {
  test.use({ viewport: { width: 900, height: 820 } });

  test("arriving at a step raises the sheet to its action, and the callout sits clear of the sheet and the hole", async ({ page }) => {
    await start(page);
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await expectClear(page, { width: 900, height: 820 });
    const [box, sheet] = [await settledBox(callout(page)), await settledBox(page.locator("aside.inspector"))];
    expect(box.y + box.height).toBeLessThanOrEqual(sheet.y + 1);
  });
});
