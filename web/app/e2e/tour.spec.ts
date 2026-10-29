// PRD: C20. A guided first run on cairn demo: the welcome opens on Home
// and never over a deep link; a practice journey is made on Start, walked through the ordinary
// screens under a spotlight, and deleted when the tour ends. What each step says is composed from
// the state the engine derived (steps.test.ts); here the oracle is the screen: where the hole and
// the callout are, what stays dim and what lifts after the action, which rows became ready, where
// the person is.
import type { BrowserContext, Locator, Page } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { setFrontier } from "./acting.ts";
import { goWithin, nodeCard, nodePanel, open, openFromCanvas, visit, whenSettled } from "./shell.ts";

const callout = (page: Page) => page.getByTestId("tour-callout");
const scrim = (page: Page) => page.getByTestId("tour-scrim");
const ring = (page: Page) => page.getByTestId("tour-ring");

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A locator's box once it has stopped moving (the callout glides, the canvas pans). */
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
  const match = /\/journeys\/(j_[^/?]+)/.exec(page.url());
  expect(match).not.toBeNull();
  return match?.[1] ?? "";
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

/** The tour is at `step`, showing its spotlight or its reveal. */
async function expectAt(page: Page, step: string, phase: "spotlight" | "reveal"): Promise<void> {
  await expect(callout(page)).toHaveAttribute("data-step", step);
  await expect(callout(page)).toHaveAttribute("data-phase", phase);
  await expect(scrim(page)).toHaveAttribute("data-lifted", String(phase === "reveal"));
  // The state is placed: what the step does to the sheet on arrival has been done.
  await expect(callout(page)).toHaveAttribute("data-ready", "true");
}

/** Walks from Home to the step that asks `step`'s action, doing the earlier steps' actions. */
async function walkTo(page: Page, step: "answer" | "frontier" | "finish" | "dates"): Promise<void> {
  await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
  if (step === "answer") {
    await expectAt(page, "answer", "spotlight");
    return;
  }
  await decide(page, "yes");
  await next(page);
  if (step === "frontier") {
    return;
  }
  await setFrontier(page, true);
  await next(page);
  if (step === "finish") {
    return;
  }
  await markDraftDone(page);
  await next(page);
}

test("the welcome opens on Home and never over a deep link", async ({ page }) => {
  await visit(page, "/journeys");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  // The rail offers it quietly; the link opens the welcome on Home.
  await page.getByTestId("rail-tour").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
  await expect(page.getByTestId("rail-tour")).toHaveCount(0);
  await visit(page, "/");
  await expect(page.getByTestId("tour-welcome")).toBeVisible();
  await checkpoint(page);
  // Not now ends the offers; the rail and You still offer the tour.
  await page.getByTestId("tour-not-now").click();
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
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

/** The practice journey `id` is deleted, the journeys are as many as before, and no offer comes back. */
async function practiceIsGone(page: Page, id: string, journeyCount: number): Promise<void> {
  // The practice journey is deleted and no offer comes back.
  await open(page, "/journeys?status=any");
  await expect(page.locator(`[data-testid="journey-row"][data-journey="${id}"]`)).toHaveCount(0);
  await expect(page.getByTestId("journey-row")).toHaveCount(journeyCount);
  await open(page, "/");
  await expect(page.getByTestId("tour-welcome")).toHaveCount(0);
  await expect(page.getByTestId("tour-continue")).toHaveCount(0);
}

test("the tour walks a practice journey, and the journey is gone at the end", async ({ page, context }) => {
  const assistant: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith("/assistant")) {
      assistant.push(request.url());
    }
  });
  await open(page, "/journeys?status=any");
  const journeyCount = await whenSettled(page, () => page.getByTestId("journey-row").count());
  const id = await start(page);
  await expect(page.getByTestId("tour-label")).toContainText("of 6");
  await sharedWithAnotherTab(context);
  // Step 1: dimmed but for the decision; Next waits for it to be selected.
  await expectAt(page, "map", "spotlight");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await expect(page.getByTestId("tour-waits")).toBeVisible();
  await checkpoint(page);
  await openFromCanvas(page, "n_outside");
  await expectAt(page, "map", "reveal");
  await expect(page.getByTestId("tour-waits")).toHaveCount(0);
  await next(page);
  // Step 2: the answer decides whether the review applies; the scrim lifts once it is saved.
  await expectAt(page, "answer", "spotlight");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await decide(page, "yes");
  await expectAt(page, "answer", "reveal");
  await checkpoint(page);
  await next(page);
  // Step 3: Frontier.
  await expectAt(page, "frontier", "spotlight");
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  await setFrontier(page, true);
  await expectAt(page, "frontier", "reveal");
  await next(page);
  // Step 4: finish the draft in the list; the work that waited for it is ready.
  await expectAt(page, "finish", "spotlight");
  await expect(page).toHaveURL(/view=list/);
  await expect(page.getByTestId("tour-next")).toHaveAttribute("aria-disabled", "true");
  // The due date is said once: the Due column has it, so the fact under the title does not repeat it.
  await expect(page.locator('tr[data-node="n_draft"] [data-testid="list-due"]')).toBeVisible();
  await expect(page.locator('tr[data-node="n_draft"] [data-testid="fact"]')).toBeHidden();
  await markDraftDone(page);
  await expectAt(page, "finish", "reveal");
  await checkpoint(page);
  // Going back keeps what was done; the earlier step arrives showing what is true now.
  await page.getByTestId("tour-back").click();
  await expectAt(page, "frontier", "reveal");
  await expect(page.getByTestId("tour-next")).not.toHaveAttribute("aria-disabled", "true");
  await next(page);
  await next(page);
  // Step 5: dates, then the end.
  await expectAt(page, "dates", "spotlight");
  await expect(page).toHaveURL(/view=timeline/);
  // The section the sentence is about is open inside the hole.
  await expect(page.getByTestId("dates")).toBeVisible();
  await checkpoint(page);
  await page.getByTestId("tour-next").click();
  // Step 6: the assistant, shown as an example, then the end.
  await expectAt(page, "assistant", "spotlight");
  await showAssistantExample(page);
  await page.getByTestId("tour-next").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("tour-end")).toBeVisible();
  await checkpoint(page);
  await page.getByTestId("tour-close").click();
  await expect(page.getByTestId("tour-end")).toHaveCount(0);
  await practiceIsGone(page, id, journeyCount);
  // The assistant step is an example: nothing was asked of the journey assistant.
  expect(assistant).toEqual([]);
});

test("a press on the dimmed page does nothing, and the editor inside the hole saves", async ({ page }) => {
  await walkTo(page, "answer");
  const chip = await settledBox(page.getByTestId("chip-frontier"));
  // Where the Frontier chip really is: the scrim takes the press.
  await page.mouse.click(chip.x + chip.width / 2, chip.y + chip.height / 2);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(callout(page)).toHaveAttribute("data-phase", "spotlight");
  // The hole holds the whole editor, so the person can choose and save without leaving it.
  const hole = await settledBox(ring(page));
  const editor = await settledBox(nodePanel(page, "n_outside").getByTestId("answer-editor"));
  expect(holds(hole, editor, 6)).toBe(true);
  await decide(page, "yes");
  await expectAt(page, "answer", "reveal");
});

// The change an answer makes, as the sentence names it: Yes brings the review in; No drops it and the summary is what moves.
for (const [option, named] of [["yes", "n_review"], ["no", "n_summary"]] as const) {
  test(`after the answer ${option} the scrim lifts and the callout covers nothing its sentence names`, async ({ page }) => {
    await walkTo(page, "answer");
    await decide(page, option);
    await expectAt(page, "answer", "reveal");
    const box = await settledBox(callout(page));
    expect(meets(box, await settledBox(nodeCard(page, named)))).toBe(false);
  });
}

test("after Done the scrim is lifted and the callout is clear of the rows it freed", async ({ page }) => {
  await walkTo(page, "finish");
  await markDraftDone(page);
  await expectAt(page, "finish", "reveal");
  const box = await settledBox(callout(page));
  for (const node of ["n_comments", "n_review"]) {
    expect(meets(box, await settledBox(page.locator(`tr[data-node="${node}"]`)))).toBe(false);
  }
  // Back and forth: Back onto the finished step arrives in its reveal, Back onto the first in its spotlight.
  await next(page);
  await expectAt(page, "dates", "spotlight");
  await page.getByTestId("tour-back").click();
  await expectAt(page, "finish", "reveal");
  for (const step of ["frontier", "answer"]) {
    await page.getByTestId("tour-back").click();
    await expect(callout(page)).toHaveAttribute("data-step", step);
  }
  await page.getByTestId("tour-back").click();
  await expectAt(page, "map", "spotlight");
});

test("the hole follows what it holds: a pan of the canvas, a scroll of the inspector", async ({ page }) => {
  await start(page);
  const card = nodeCard(page, "n_outside");
  const before = await settledBox(card);
  expect(holds(await settledBox(ring(page)), before, 8)).toBe(true);
  // Drag the canvas by the card the ring is on.
  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(before.x + before.width / 2 - 60, before.y + before.height / 2 + 30, { steps: 6 });
  await page.mouse.up();
  const after = await settledBox(card);
  expect(Math.abs(after.x - before.x) + Math.abs(after.y - before.y)).toBeGreaterThan(20);
  await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(card), 8)).toBe(true);
  await openFromCanvas(page, "n_outside");
  await next(page);
  // Scroll the inspector under the hole: the ring stays on the editor.
  const editor = nodePanel(page, "n_outside").getByTestId("answer-editor");
  const at = await settledBox(editor);
  await page.mouse.move(at.x + at.width / 2, at.y + at.height / 2);
  await page.mouse.wheel(0, 60);
  await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(editor), 8)).toBe(true);
});

test("a card panned out of the canvas is brought back by Show the control", async ({ page }) => {
  await start(page);
  const card = nodeCard(page, "n_outside");
  // Drag the card's visible edge to the window's edge until it has all but left.
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
  await expect(page.getByTestId("tour-show")).toBeVisible();
  await page.getByTestId("tour-show").click();
  await expect.poll(async () => (await settledBox(card)).x + (await settledBox(card)).width).toBeLessThan(1280);
  await expect(page.getByTestId("tour-show")).toHaveCount(0);
});

test("keys under the dim do nothing, and Escape never ends the tour", async ({ page }) => {
  await start(page);
  const url = page.url();
  await page.keyboard.press("f");
  await page.keyboard.press("?");
  await expect(page.locator("dialog[open]")).toHaveCount(0);
  expect(page.url()).toBe(url);
  await page.keyboard.press("Escape");
  await expect(callout(page)).toBeVisible();
  await expect(page.getByTestId("tour-title")).toBeFocused();
});

test("with a menu open, Escape closes the menu and only that", async ({ page }) => {
  await walkTo(page, "answer");
  const menu = page.locator('[data-popover]:not([data-testid="tour-callout"])');
  await nodePanel(page, "n_outside").getByTestId("more-actions").click();
  await expect(menu).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(callout(page)).toBeVisible();
  await expect(callout(page)).toHaveAttribute("data-phase", "spotlight");
});

test("a dialog over the tour has the focus, and the tour does not take it back", async ({ page }) => {
  await start(page);
  await page.evaluate(() => {
    const dialog = document.createElement("dialog");
    dialog.id = "over-the-tour";
    dialog.innerHTML = '<input aria-label="Name" /><button>Close</button>';
    document.body.append(dialog);
    dialog.showModal();
  });
  await expect(page.getByLabel("Name")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect.poll(() => page.evaluate(() => document.activeElement?.closest("#over-the-tour") !== null)).toBe(true);
  await page.evaluate(() => { document.querySelector("#over-the-tour")?.remove(); });
  await expect(callout(page)).toBeVisible();
});

test("Skip returns Home and the practice journey is deleted", async ({ page }) => {
  const id = await start(page);
  await openFromCanvas(page, "n_outside");
  await next(page);
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

test.describe("under 1100px, where the inspector is a sheet", () => {
  test.use({ viewport: { width: 900, height: 820 } });

  test("arriving at a step raises the sheet to its action, every time", async ({ page }) => {
    await start(page);
    await openFromCanvas(page, "n_outside");
    await next(page);
    const frame = page.locator(".frame");
    await expectAt(page, "answer", "spotlight");
    await expect(frame).toHaveAttribute("data-sheet", "half");
    // Back and forward again, with the decision still open, puts the sheet up for the action once more.
    await page.getByTestId("tour-back").click();
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expectAt(page, "answer", "spotlight");
    await expect(frame).toHaveAttribute("data-sheet", "half");
    await decide(page, "yes");
    await expectAt(page, "answer", "reveal");
    await expect(frame).toHaveAttribute("data-sheet", "peek");
    // Answered, the step arrives in its reveal: the sheet is down for the change.
    await page.getByTestId("tour-back").click();
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expectAt(page, "answer", "reveal");
    await expect(frame).toHaveAttribute("data-sheet", "peek");
    await checkpoint(page);
  });

  test("an action scrolled out of the sheet is brought back by Show the control", async ({ page }) => {
    await walkTo(page, "answer");
    const editor = nodePanel(page, "n_outside").getByTestId("answer-editor");
    await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(editor), 8)).toBe(true);
    // Scroll the inspector's own scroller until the editor is wholly above its view.
    await editor.evaluate((element) => {
      for (let each: HTMLElement | null = element.parentElement; each !== null; each = each.parentElement) {
        if (each.scrollHeight > each.clientHeight && getComputedStyle(each).overflowY !== "visible") {
          each.scrollTop = each.scrollHeight;
        }
      }
    });
    await expect(page.getByTestId("tour-show")).toBeVisible();
    await page.getByTestId("tour-show").click();
    await expect(page.getByTestId("tour-show")).toHaveCount(0);
    await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(editor), 8)).toBe(true);
  });

  test("the callout sits clear of the sheet and of the hole", async ({ page }) => {
    await walkTo(page, "answer");
    const box = await settledBox(callout(page));
    const sheet = await settledBox(page.locator("aside.inspector"));
    expect(box.y + box.height).toBeLessThanOrEqual(sheet.y + 1);
    expect(meets(box, await settledBox(ring(page)))).toBe(false);
  });
});

test.describe("on a phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the callout is docked clear of the hole, the sheet rises for the answer, and Frontier is reached through Controls", async ({ page }) => {
    await start(page);
    await checkpoint(page);
    const clear = async () => {
      const box = await settledBox(callout(page));
      expect(meets(box, await settledBox(ring(page)))).toBe(false);
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(390);
    };
    await clear();
    await openFromCanvas(page, "n_outside");
    await next(page);
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await clear();
    await decide(page, "yes");
    // The pane is inert at the peek: the sheet lowers and the reveal is on the screen.
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "peek");
    await expectAt(page, "answer", "reveal");
    await next(page);
    // The hole holds Controls and, once opened, the menu with Frontier in it.
    await expect(page.getByTestId("controls-button")).toBeVisible();
    await clear();
    // While the menu is open the callout steps aside, and the hole holds the whole menu; it returns when the menu closes.
    await page.getByTestId("controls-button").click();
    const menu = page.getByTestId("controls-button-menu");
    await expect(menu).toBeVisible();
    await expect(callout(page)).toBeHidden();
    await expect.poll(async () => holds(await settledBox(ring(page)), await settledBox(menu), 8)).toBe(true);
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(callout(page)).toBeVisible();
    await setFrontier(page, true);
    await expectAt(page, "frontier", "reveal");
    await next(page);
    await clear();
    await markDraftDone(page);
    await expectAt(page, "finish", "reveal");
    // Everything the sentence names is clear of the callout, which docks at the head when the foot would cover it.
    const box = await settledBox(callout(page));
    for (const node of ["n_comments", "n_review"]) {
      expect(meets(box, await settledBox(page.locator(`tr[data-node="${node}"]`)))).toBe(false);
    }
    await expect(page.getByTestId("tour-next")).toBeVisible();
  });

  test("when the sheet is lowered the callout says so and offers to raise it", async ({ page }) => {
    await walkTo(page, "answer");
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    // The handle is inside the hole, so the person can lower the sheet; the arrow keys step it down.
    await page.getByRole("button", { name: "Resize the inspector" }).focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "peek");
    await expect(page.getByTestId("tour-show")).toBeVisible();
    await page.getByTestId("tour-show").click();
    await expect(page.locator(".frame")).toHaveAttribute("data-sheet", "half");
    await expect(page.getByTestId("tour-show")).toHaveCount(0);
  });
});

test.describe("on a phone on its side", () => {
  test.use({ viewport: { width: 844, height: 390 }, hasTouch: true, isMobile: true });

  test("the callout is beside the card it is about, clear of it and inside the window", async ({ page }) => {
    await start(page);
    // The canvas frames the card once the callout has taken its side; the two settle together.
    await expect.poll(async () => meets(await settledBox(callout(page)), await settledBox(ring(page)))).toBe(false);
    const box = await settledBox(callout(page));
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y + box.height).toBeLessThanOrEqual(390);
    // The card is large enough to read what it says: its title is not a smudge.
    expect((await settledBox(nodeCard(page, "n_outside"))).width).toBeGreaterThan(150);
  });
});
