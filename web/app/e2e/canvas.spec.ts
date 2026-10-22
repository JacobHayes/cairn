// The journey canvas over the vendor evaluation (C2 to C7): the flow of reading the graph in a
// real browser, which only a browser can check (SVG hit-testing, the wheel and its modifiers,
// the viewport). It opens fitted at every detail, expands in place, traces a selection, follows an
// edge, and pans and zooms. What the canvas draws (its levels, lines, lens, layout) is the
// model's tests'.
import type { Page } from "@playwright/test";

import { checkpoint, expect, test } from "./test.ts";
import { openTip } from "./tips.ts";

import { expectTitleLinesAsLaidOut, lineBetween, pointOn, viewportOf } from "./canvas.ts";
import { journeyName, startJourney } from "./around.ts";
import { goWithin, nodeCard, nodePanel, openJourney, setDepth, setFade, settled, visit, whenSettled } from "./shell.ts";

/** The open Filter counts what it hides beside its box, and showing the not relevant removes the count. */
async function filterCountsWhatItHides(page: Page): Promise<void> {
  const cards = page.getByTestId("node-card");
  const resting = await whenSettled(page, () => cards.count());
  await page.getByTestId("filter-button").click();
  await expect(page.getByTestId("show-not-relevant-count")).toBeVisible();
  await page.getByTestId("show-not-relevant").click();
  await expect(page.getByTestId("show-not-relevant-count")).toHaveCount(0);
  await page.getByTestId("filter-button").click();
  await expect.poll(() => cards.count()).toBeGreaterThan(resting);
  await page.getByTestId("filter-button").click();
  await page.getByTestId("show-not-relevant").click();
  await expect(page.getByTestId("show-not-relevant-count")).toBeVisible();
  await page.getByTestId("filter-button").click();
  await expect.poll(() => cards.count()).toBe(resting);
}

/** At Stages the stage holding the work is open and says so, the others fold; a folded stage traces what is folded into it, and expanding opens it in place; selecting a node traces it, and Reveal draws what the step folds away. */
async function expandsAndTraces(page: Page): Promise<void> {
  await setDepth(page, "Stages");
  await expect(nodeCard(page, "n_reporting").getByTestId("card-current")).toBeAttached();
  await page.keyboard.press("f");
  await expect(nodeCard(page, "n_setup").getByTestId("card-expand")).toBeVisible();
  await expect(page.locator(".react-flow__attribution")).toHaveCount(0);
  // A folded stage traces what the nodes folded into it trace: the line leaving it is followed.
  await nodeCard(page, "n_setup").getByTestId("card-open").click();
  await expect(page.getByTestId("card-mark").filter({ hasText: "unblocks" }).first()).toBeVisible();
  // Expanding opens it in place, and the address remembers.
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await expect(page).toHaveURL(/open=n_setup/);
  // Selecting a node traces it: what it needs and what it unblocks are tagged.
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(page.getByTestId("card-mark").filter({ hasText: "needs" }).first()).toBeVisible();
  await expect(page.getByTestId("card-mark").filter({ hasText: "unblocks" }).first()).toBeVisible();
  // What the step folds away is counted, and Reveal draws it until Hide takes it off again.
  await expect(page.getByTestId("trace-reveal")).toHaveText("Reveal 2 linked nodes");
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveText("Hide revealed nodes");
}

/** Hovering an edge says what it is; clicking it opens its card, which follows it to either end. */
async function followsAnEdge(page: Page): Promise<void> {
  const edge = lineBetween(page, "n_access", "n_plan");
  await settled(page);
  const at = await pointOn(edge);
  await page.mouse.move(at.x, at.y);
  // The line's chip is the app's one tooltip, and it is the line's accessible name too (the browser's tooltip is excluded on every page, e2e/tips.ts).
  await expect(openTip(page)).toContainText("Test plan needs Environment access");
  await page.mouse.click(at.x, at.y);
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await page.getByRole("link", { name: "Go to Environment access" }).click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
}

/** A wheel pans and never zooms, and with the control key it zooms; a right-click or the menu key opens the node's menu, and a choice that needs a form selects the node and opens it there. */
async function pansZoomsAndOpensMenus(page: Page): Promise<void> {
  const [start, pointer] = [await viewportOf(page), { x: 600, y: 500 }];
  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.wheel(0, 120);
  await expect.poll(async () => (await viewportOf(page)).y).not.toBe(start.y);
  expect((await viewportOf(page)).zoom).toBe(start.zoom);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");
  await expect.poll(async () => (await viewportOf(page)).zoom).not.toBe(start.zoom);
  await nodeCard(page, "n_plan").click({ button: "right" });
  await expect(page.getByTestId("context-menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("context-menu")).toHaveCount(0);
  await nodeCard(page, "n_plan").getByTestId("card-open").focus();
  // With the inspector collapsed, a choice that opens a form brings it back.
  await page.keyboard.press("]");
  await expect(page.getByTestId("node-detail")).toBeHidden();
  await page.keyboard.press("Shift+F10");
  await page.getByTestId("context-rename").click();
  await expect(nodePanel(page, "n_plan").getByTestId("rename")).toBeVisible();
}

/**
 * On a journey just started, its up-front decisions are the frontier at its top level, which Stages leaves out: Frontier only
 * emphasises (it draws the same cards), Depth counts the frontier work it hides, and Fade draws that work again, dimmed.
 */
async function depthCountsHiddenFrontier(page: Page): Promise<void> {
  const cards = page.getByTestId("node-card");
  const id = await startJourney(page, "browser", journeyName("Depth"), { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", id, "?frontier=1");
  const frontierCards = page.locator('[data-testid="node-card"][data-here*="actionable now"]');
  const [drawn, frontier] = await whenSettled(page, () => Promise.all([cards.count(), frontierCards.evaluateAll((found) => found.map((card) => card.getAttribute("data-node") ?? ""))]));
  expect(frontier.length).toBeGreaterThan(0);
  await expect(page.getByTestId("depth-badge")).toHaveCount(0);
  await setDepth(page, "Stages");
  await expect.poll(() => cards.count()).toBeLessThan(drawn);
  const left = await whenSettled(page, () => frontierCards.evaluateAll((found) => found.map((card) => card.getAttribute("data-node") ?? "")));
  const hidden = frontier.filter((key) => !left.includes(key));
  expect(hidden.length).toBeGreaterThan(0);
  await expect(page.getByTestId("depth-badge")).toHaveText(String(hidden.length));
  await setFade(page, true);
  await expect.poll(() => cards.count()).toBe(drawn);
  await expect(page.getByTestId("depth-badge")).toHaveCount(0);
  for (const key of hidden) {
    await expect(nodeCard(page, key)).toHaveClass(/node-faded/);
  }
}

test("C2 to C7: the graph opens fitted at every depth, expands in place, traces a selection, follows an edge, and pans and zooms", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  // It opens at the stock view: every depth, fitted to the whole graph, as Fit does.
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Actions");
  await expectTitleLinesAsLaidOut(page);
  await filterCountsWhatItHides(page);
  await checkpoint(page);
  await expandsAndTraces(page);
  await followsAnEdge(page);
  await pansZoomsAndOpensMenus(page);
  await checkpoint(page);
  await depthCountsHiddenFrontier(page);
});

/** Click selects and shift-click adds, the inspector lists the members, and the actions that fit them all are offered. */
async function selectsASet(page: Page): Promise<void> {
  const members = page.getByTestId("selection-member");
  await nodeCard(page, "n_access").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  // Shift on a title adds as it does on the body; two members are listed, with no trace left drawn.
  await nodeCard(page, "n_plan").getByTestId("card-open").click({ modifiers: ["Shift"] });
  await nodeCard(page, "n_decision_meeting").click({ modifiers: ["Shift"] });
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "3");
  await expect(members).toHaveCount(3);
  await expect(page.getByTestId("node-detail")).toHaveCount(0);
  await expect(page.getByTestId("card-mark")).toHaveCount(0);
  // Start fits an action but not a milestone: shown, disabled, with who cannot.
  await page.getByTestId("selection-status").click();
  const start = page.getByRole("menuitem", { name: "Start" });
  await expect(start).toBeDisabled();
  await expect(start).toHaveAccessibleDescription(/cannot start/);
  await page.keyboard.press("Escape");
  // A member dropped with its × leaves the rest; a plain click on one card is that card alone.
  await members.filter({ hasText: "decision" }).first().getByRole("button", { name: /Remove .* from the selection/ }).click();
  await expect(members).toHaveCount(2);
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
}

/** Esc clears several to nothing and a cleared set stays cleared; a shift-drag over empty canvas selects the cards in its box. */
async function clearsAndBoxSelects(page: Page): Promise<void> {
  await nodeCard(page, "n_access").click({ modifiers: ["Shift"] });
  await expect(page.getByTestId("selection-panel")).toBeVisible();
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await expect(page).not.toHaveURL(/\/nodes\//);
  // Back to the node the set was made at opens that node alone: a cleared set stays cleared.
  await page.goBack();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  await expect(page.getByTestId("node-detail")).toBeVisible();
  await settled(page);
  const [from, to] = await whenSettled(page, () => Promise.all([nodeCard(page, "n_purpose").boundingBox(), nodeCard(page, "n_who_owns").boundingBox()]));
  if (from === null || to === null) {
    throw new Error("the cards have no box");
  }
  await page.keyboard.down("Shift");
  await page.mouse.move(Math.min(from.x, to.x) - 12, Math.min(from.y, to.y) - 12);
  await page.mouse.down();
  await page.mouse.move(Math.max(from.x + from.width, to.x + to.width) + 10, Math.max(from.y + from.height, to.y + to.height) + 10, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up("Shift");
  await expect(page.getByTestId("selection-panel")).toBeVisible();
  expect(await whenSettled(page, () => page.getByTestId("selection-member").count())).toBeGreaterThanOrEqual(2);
}

/** A click on the empty canvas deselects, and a drag that pans does not. */
async function emptyCanvasDeselects(page: Page): Promise<void> {
  await nodeCard(page, "n_reporting").getByTestId("card-open").click();
  await expect(page).toHaveURL(/\/nodes\/n_reporting/);
  const pane = (await whenSettled(page, () => page.locator(".react-flow__pane").boundingBox())) ?? { x: 0, y: 0, width: 0, height: 0 };
  const corner = { x: pane.x + pane.width - 12, y: pane.y + pane.height - 60 };
  await page.mouse.move(corner.x, corner.y);
  await page.mouse.down();
  await page.mouse.move(corner.x - 40, corner.y - 30);
  await page.mouse.up();
  await expect(page).toHaveURL(/\/nodes\/n_reporting/);
  // A click straight after a pan is swallowed by the pan; wait for it to end.
  await settled(page);
  await page.mouse.click(corner.x, corner.y);
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page.getByTestId("card-mark")).toHaveCount(0);
}

/** A menu's arrow keys move over the items that can take the focus, never stalling at a disabled one. */
async function menuArrowsSkipDisabled(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_bakeoff");
  await nodeCard(page, "n_trial_a").getByTestId("card-open").click();
  await nodeCard(page, "n_wrap_up").click({ modifiers: ["Shift"] });
  await expect(page.getByTestId("selection-panel")).toHaveAttribute("data-count", "2");
  // The trial is already active, so Start is disabled; the other items of the Status menu are not.
  const button = page.getByTestId("selection-status");
  await button.focus();
  await page.keyboard.press("Enter");
  const menu = page.getByTestId("selection-status-menu");
  await expect(menu.getByRole("menuitem", { name: "Start" })).toBeDisabled();
  const enabled = menu.locator('[role="menuitem"]:not(:disabled)');
  const [count, names] = await whenSettled(page, () => Promise.all([enabled.count(), enabled.allInnerTexts()]));
  expect(count).toBeGreaterThan(1);
  // Opened from the keyboard, the focus is on the first enabled item; down all the way round returns to it.
  for (let step = 0; step <= count; step += 1) {
    await expect(menu.locator('[role="menuitem"]:focus')).toHaveCount(1);
    await expect(menu.locator('[role="menuitem"]:disabled:focus')).toHaveCount(0);
    await expect(menu.locator('[role="menuitem"]:focus')).toHaveText(names[step % count] ?? "");
    await page.keyboard.press("ArrowDown");
  }
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(button).toBeFocused();
}

test("selection is one set with no mode: click selects, shift-click adds, the inspector lists the members and the actions that fit them all", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await selectsASet(page);
  await clearsAndBoxSelects(page);
  await checkpoint(page);
  await emptyCanvasDeselects(page);
  await checkpoint(page);
  await menuArrowsSkipDisabled(page);
});

/** Under Fade the cards below the depth are 0.3 whatever their state, a container holding a decision stays clear, and Frontier's dimming joins it without multiplying. */
async function fadeAndFrontierLevels(page: Page): Promise<void> {
  const read = () =>
    whenSettled(page, () => page.getByTestId("node-card").evaluateAll((found) =>
      found.map((card) => ({
        key: card.getAttribute("data-node") ?? "",
        kind: card.getAttribute("data-kind") ?? "",
        parent: card.getAttribute("data-parent"),
        done: card.classList.contains("node-done"),
        here: card.getAttribute("data-here") ?? "",
        opacity: Number(getComputedStyle(card).opacity),
      })),
    ));
  const cards = await read();
  const held = new Set(cards.filter((card) => card.kind === "decision").flatMap((card) => {
    const above: string[] = [];
    for (let at = card.parent; at !== null; at = cards.find((each) => each.key === at)?.parent ?? null) {
      above.push(at);
    }
    return above;
  }));
  const below = (card: (typeof cards)[number]) => (card.kind === "deliverable" || card.kind === "action") && !held.has(card.key);
  // The fixture has what the rule needs to be seen: work below the depth, done and open, and a deliverable holding a decision or a group in focus.
  expect(cards.filter((card) => below(card) && card.done).length).toBeGreaterThan(0);
  expect(cards.filter((card) => below(card) && !card.done).length).toBeGreaterThan(0);
  for (const card of cards) {
    expect(card.opacity, `${card.key} (${card.kind}${card.done ? ", done" : ""})`).toBeCloseTo(below(card) ? 0.3 : card.done ? 0.5 : 1);
  }
  // With Frontier on as well, a card out of either focus is 0.3, never 0.3 times 0.3.
  await page.getByTestId("chip-frontier").click();
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(async () => (await read()).some((card) => !below(card) && !card.here.includes("actionable now") && card.opacity < 0.5)).toBe(true);
  for (const card of await read()) {
    expect(card.opacity, card.key).toBeGreaterThanOrEqual(0.3 - 1e-9);
    expect([0.3, 0.5, 1].some((level) => Math.abs(card.opacity - level) < 1e-6), `${card.key} is at a level`).toBe(true);
  }
  await checkpoint(page);
}

/** Reveal's nodes are dimmed by the depth under Fade as the rest are, never lifted by the trace (Frontier off, so it supplies no opacity). */
async function revealedNodesStayDimmed(page: Page): Promise<void> {
  await page.getByTestId("chip-frontier").click();
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await nodeCard(page, "n_partner_runs").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-reveal")).toBeVisible();
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveText("Hide revealed nodes");
  for (const key of ["n_criteria", "n_partner_results"]) {
    await expect(nodeCard(page, key), key).toBeVisible();
    await expect(nodeCard(page, key), key).toHaveCSS("opacity", "0.3");
  }
  await checkpoint(page);
}

/** At the stock view, open and in focus is full and done is half; a zoomed-out stage head draws its chip inside the head, a group's has none; a trace puts what is outside it at a third. */
async function stageHeadsAndTracedOpacity(page: Page): Promise<void> {
  await goWithin(page, "/journeys/j_vendor_eval?view=graph&depth=actions");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await page.keyboard.press("f");
  await expect(nodeCard(page, "n_reporting")).toHaveCSS("opacity", "1");
  await expect(nodeCard(page, "n_testing")).toHaveCSS("opacity", "0.5");
  const chip = nodeCard(page, "n_testing").getByTestId("card-head-chip");
  await expect(chip).toBeVisible();
  const [chipBox, cardBox] = await whenSettled(page, () => Promise.all([chip.boundingBox(), nodeCard(page, "n_testing").boundingBox()]));
  if (chipBox === null || cardBox === null) {
    throw new Error("the stage and its chip are drawn");
  }
  expect(chipBox.y).toBeGreaterThanOrEqual(cardBox.y);
  expect(chipBox.x + chipBox.width).toBeLessThanOrEqual(cardBox.x + cardBox.width);
  await expect(nodeCard(page, "n_final_review").getByTestId("card-head-chip")).toHaveCount(0);
  // Tracing a card puts what is outside the trace at a third, done or not: the levels are never multiplied.
  await nodeCard(page, "n_final_review").getByTestId("card-open").click({ force: true });
  await expect(nodeCard(page, "n_purpose")).toHaveCSS("opacity", "0.3");
  // The final review closes at the decision meeting: that card is the far end of its date line, so it stays in focus.
  await expect(nodeCard(page, "n_decision_meeting")).toHaveCSS("opacity", "1");
  await expect(nodeCard(page, "n_kickoff")).toHaveCSS("opacity", "0.5");
}

test("C2, DESIGN opacity: a card takes the lowest opacity that applies: under Fade the cards below the depth are 0.3 whatever their state, a container holding a decision stays clear, Frontier's dimming joins it without multiplying, and a trace's never lifts it", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openJourney(page, "browser", "j_vendor_eval", "?depth=decisions&fade=1");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await fadeAndFrontierLevels(page);
  await checkpoint(page);
  await revealedNodesStayDimmed(page);
  await checkpoint(page);
  await stageHeadsAndTracedOpacity(page);
});

/** A line between folded stages opens the links it stands for, and each end is a link. */
async function stagesLineOpensLinks(page: Page): Promise<void> {
  await visit(page, "browser", "/journeys/j_vendor_eval/edges/n_setup~n_testing?depth=stages");
  await expect(page.getByTestId("edge-missing")).toHaveCount(0);
  const card = page.getByTestId("edge-card");
  await expect(card).toBeVisible();
  const row = card.locator('[data-requirement="n_plan"][data-dependent="n_comparison_set"]');
  await expect(row).toHaveAttribute("data-waits", /^(true|false)$/);
  await expect(lineBetween(page, "n_setup", "n_testing")).toHaveClass(/line-selected/);
  await row.getByRole("link", { name: "Comparison set" }).click();
  await expect(nodePanel(page, "n_comparison_set")).toBeVisible();
}

test("C1, C2: a line is selectable only where the view's own level draws it, an edge address is always the Graph, and a line between folded stages opens the links it stands for, each end a link", async ({ page }) => {
  await stagesLineOpensLinks(page);
  // Test plan is open at Stages, so its stage is opened for it: the lines inside that stage are drawn only for the selection, and open no card.
  const selectable = (key: string, other: string) => lineBetween(page, key, other).locator("xpath=ancestor::*[contains(@class,'react-flow__edge')][1]");
  await visit(page, "browser", "/journeys/j_vendor_eval/nodes/n_plan?depth=stages");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await expect(selectable("n_access", "n_plan")).not.toHaveClass(/\bselectable\b/);
  // With nothing selected the stages' own line is the view's, and opens one.
  await visit(page, "browser", "/journeys/j_vendor_eval?depth=stages&view=graph");
  await expect(selectable("n_setup", "n_testing")).toHaveClass(/\bselectable\b/);
  // A line address on the List's query is the Graph, with its card; a depth that folds its ends says the view draws no line, and both ends stay one click away.
  await visit(page, "browser", "/journeys/j_vendor_eval/edges/n_access~n_plan?view=list");
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await expect(page.getByTestId("journey-frame")).toHaveAttribute("data-screen", "graph");
  await setDepth(page, "Stages");
  await expect(page.getByTestId("edge-missing")).toBeVisible();
  await expect(page.getByRole("link", { name: "Go to Environment access" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Go to Test plan" })).toBeVisible();
});

test("C1: date relationships are drawn only for the one selected node, both ways, with an arrowhead where the order is fixed, open no card, and are listed in the inspector's linked dates", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_launch?view=graph&depth=actions");
  await expect(nodeCard(page, "n_launch")).toBeVisible();
  const dates = page.locator('[data-kind="dates"]');
  // At rest no date relationship is drawn.
  await expect(dates).toHaveCount(0);
  // Documentation is due before the launch: one line, with the later end marked.
  await nodeCard(page, "n_docs").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_docs")).toBeVisible();
  await expect(dates).toHaveCount(1);
  await expect(dates.first()).toHaveAttribute("data-marker", "open");
  const address = page.url();
  await dates.first().dispatchEvent("click");
  expect(page.url()).toBe(address);
  // The launch has five relationships; the trace fades what it does not reach to a third, and none of the far ends is.
  await nodeCard(page, "n_launch").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_launch")).toBeVisible();
  await expect(dates).toHaveCount(5);
  for (const far of ["n_docs", "n_announcement", "n_go_no_go", "n_code_freeze", "n_retro"]) {
    await expect(nodeCard(page, far), far).not.toHaveCSS("opacity", "0.3");
  }
  // The inspector lists them, and a link in it selects that node.
  const linked = page.getByTestId("linked-dates");
  await expect(linked).toBeVisible();
  await linked.locator("summary").click();
  await expect(linked.locator("li")).toHaveCount(5);
  await linked.locator('li[data-node="n_docs"] a').first().click();
  await expect(nodePanel(page, "n_docs")).toBeVisible();
  await expect(dates).toHaveCount(1);
  // Deselecting clears them, and a multi-selection draws none.
  await page.keyboard.press("Escape");
  await expect(dates).toHaveCount(0);
  await nodeCard(page, "n_launch").getByTestId("card-open").click();
  await expect(dates).toHaveCount(5);
  await nodeCard(page, "n_docs").click({ modifiers: ["Shift"] });
  await expect(dates).toHaveCount(0);
});

test("C1: a date line's far end stays in focus under Frontier, and a published route's inspector lists the same relationships", async ({ page }) => {
  await visit(page, "browser", "/journeys/j_launch?view=graph&depth=actions");
  await expect(nodeCard(page, "n_launch")).toBeVisible();
  await page.getByTestId("chip-frontier").click();
  // Launch is not on the frontier, so Frontier dims it; Documentation's line to it keeps it in focus.
  await expect(nodeCard(page, "n_launch")).toHaveCSS("opacity", "0.3");
  await nodeCard(page, "n_docs").getByTestId("card-open").click();
  await expect(page.locator('[data-kind="dates"]')).toHaveCount(1);
  await expect(nodeCard(page, "n_launch")).not.toHaveCSS("opacity", "0.3");
  // A route's version reads the same relationships in its inspector.
  await visit(page, "browser", "/routes/product-launch/nodes/n_launch?version=1&view=graph");
  const linked = page.getByTestId("linked-dates");
  await expect(linked).toBeVisible();
  await linked.locator("summary").click();
  await expect(linked.locator("li")).toHaveCount(5);
  await expect(page.locator('[data-kind="dates"]')).toHaveCount(5);
});
