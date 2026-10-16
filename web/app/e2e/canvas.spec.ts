// The journey canvas over the vendor evaluation (C2 to C7): the flow of reading the graph in a
// real browser, which only a browser can check (SVG hit-testing, the wheel and its modifiers,
// the viewport). It opens fitted at every detail, expands in place, traces a selection, follows an
// edge, and pans and zooms. What the canvas draws (its levels, lines, lens, layout) is the
// model's tests'.
import { expect, test } from "@playwright/test";

import { lineBetween, pointOn, viewportOf, viewportSettled } from "./canvas.ts";
import { journeyName, startJourney } from "./around.ts";
import { nodeCard, nodePanel, openJourney, setDepth, setFade } from "./shell.ts";

test("C2 to C7: the graph opens fitted at every depth, expands in place, traces a selection, follows an edge, and pans and zooms", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  // It opens at the stock view: every depth, fitted to the whole graph, as Fit does.
  await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Actions");
  // At Stages the stage holding the work is open and says so, the others fold.
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
  // Selecting a node traces it: what it needs and what it unblocks are tagged,.
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(page.getByTestId("card-mark").filter({ hasText: "needs" }).first()).toBeVisible();
  await expect(page.getByTestId("card-mark").filter({ hasText: "unblocks" }).first()).toBeVisible();
  // What the step folds away is counted, and Reveal draws it until Hide takes it off again.
  await expect(page.getByTestId("trace-reveal")).toHaveText("Reveal 2 linked nodes");
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveText("Hide revealed nodes");
  // Hovering an edge says what it is; clicking it opens its card, which follows it to either end.
  const edge = lineBetween(page, "n_access", "n_plan");
  await viewportSettled(page);
  const at = await pointOn(edge);
  await page.mouse.move(at.x, at.y);
  await expect(page.getByTestId("edge-hover")).toContainText("Test plan needs Environment access");
  await page.mouse.click(at.x, at.y);
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await page.getByRole("link", { name: "Go to Environment access" }).click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  // A wheel pans and never zooms; with the control key it zooms.
  const [start, pointer] = [await viewportOf(page), { x: 600, y: 500 }];
  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.wheel(0, 120);
  await expect.poll(async () => (await viewportOf(page)).y).not.toBe(start.y);
  expect((await viewportOf(page)).zoom).toBe(start.zoom);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");
  await expect.poll(async () => (await viewportOf(page)).zoom).not.toBe(start.zoom);
  // A right-click, or the menu key on a focused card, opens the node's menu; a choice that needs a form selects the node and opens it there.
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
});

test("a click on the empty canvas deselects and a drag that pans does not", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  await nodeCard(page, "n_reporting").getByTestId("card-open").click();
  await expect(page).toHaveURL(/\/nodes\/n_reporting/);
  const pane = (await page.locator(".react-flow__pane").boundingBox()) ?? { x: 0, y: 0, width: 0, height: 0 };
  const corner = { x: pane.x + pane.width - 12, y: pane.y + pane.height - 60 };
  await page.mouse.move(corner.x, corner.y);
  await page.mouse.down();
  await page.mouse.move(corner.x - 40, corner.y - 30);
  await page.mouse.up();
  await expect(page).toHaveURL(/\/nodes\/n_reporting/);
  // A click straight after a pan is swallowed by the pan; wait for it to end.
  await viewportSettled(page);
  await page.mouse.click(corner.x, corner.y);
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page.getByTestId("card-mark")).toHaveCount(0);
});

test("selection is one set with no mode: click selects, shift-click adds, the inspector lists the members and the actions that fit them all", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
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
  await expect(start).toHaveAttribute("title", /cannot start/);
  await page.keyboard.press("Escape");
  // A member dropped with its × leaves the rest; a plain click on one card is that card alone.
  await members.filter({ hasText: "decision" }).first().getByRole("button", { name: /Remove .* from the selection/ }).click();
  await expect(members).toHaveCount(2);
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await expect(page.getByTestId("selection-panel")).toHaveCount(0);
  // Esc clears several to nothing.
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
  // A shift-drag over empty canvas selects the cards in its box.
  await viewportSettled(page);
  const [from, to] = [await nodeCard(page, "n_purpose").boundingBox(), await nodeCard(page, "n_who_owns").boundingBox()];
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
  expect(await members.count()).toBeGreaterThanOrEqual(2);
});

test("the controls row: Filter wears a badge for what it hides, Frontier only emphasises", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  const cards = page.getByTestId("node-card");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  const before = await cards.count();
  // Not-relevant nodes are left out by default, and the badge says how many; showing them removes it.
  await expect(page.getByTestId("filter-badge")).toBeVisible();
  await expect(page.getByTestId("filter-button")).toHaveAttribute("title", /not relevant hidden/);
  await page.getByTestId("filter-button").click();
  await page.getByTestId("show-not-relevant").click();
  await expect(page.getByTestId("filter-badge")).toHaveCount(0);
  await page.getByTestId("filter-button").click();
  await expect.poll(() => cards.count()).toBeGreaterThan(before);
  // Frontier highlights what can move now and draws the same cards.
  const drawn = await cards.count();
  await page.getByTestId("chip-frontier").click();
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  expect(await cards.count()).toBe(drawn);
  await expect(page.getByTestId("depth-badge")).toHaveCount(0);
});

test("a card takes the lowest opacity that applies, and a zoomed-out stage head keeps its chip", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openJourney(page, "browser", "j_vendor_eval", "");
  await page.keyboard.press("f");
  // Open and in focus is full, done is half; a zoomed-out stage head draws its chip inside the head, a group's has none.
  await expect(nodeCard(page, "n_reporting")).toHaveCSS("opacity", "1");
  await expect(nodeCard(page, "n_testing")).toHaveCSS("opacity", "0.5");
  const chip = nodeCard(page, "n_testing").getByTestId("card-head-chip");
  await expect(chip).toBeVisible();
  const [chipBox, cardBox] = [await chip.boundingBox(), await nodeCard(page, "n_testing").boundingBox()];
  if (chipBox === null || cardBox === null) {
    throw new Error("the stage and its chip are drawn");
  }
  expect(chipBox.y).toBeGreaterThanOrEqual(cardBox.y);
  expect(chipBox.x + chipBox.width).toBeLessThanOrEqual(cardBox.x + cardBox.width);
  await expect(nodeCard(page, "n_final_review").getByTestId("card-head-chip")).toHaveCount(0);
  // Tracing a card puts what is outside the trace at a third, done or not: the levels are never multiplied.
  await nodeCard(page, "n_final_review").getByTestId("card-open").click({ force: true });
  await expect(nodeCard(page, "n_decision_meeting")).toHaveCSS("opacity", "0.3");
  await expect(nodeCard(page, "n_purpose")).toHaveCSS("opacity", "0.3");
  await expect(nodeCard(page, "n_kickoff")).toHaveCSS("opacity", "0.5");
});

test("C2: Depth hides what is below it and counts the frontier work that hides; Fade draws every card and dims those below the depth", async ({ page }) => {
  // A journey just started: its up-front decisions are the frontier, and they sit at its top level, which Stages leaves out.
  const id = await startJourney(page, "browser", journeyName("Depth"), { route: "vendor-evaluation", version: 1 });
  await openJourney(page, "browser", id, "?frontier=1");
  const cards = page.getByTestId("node-card");
  const drawn = await cards.count();
  const frontierCards = page.locator('[data-testid="node-card"][data-here*="actionable now"]');
  const frontier = await frontierCards.evaluateAll((found) => found.map((card) => card.getAttribute("data-node") ?? ""));
  expect(frontier.length).toBeGreaterThan(0);
  await expect(page.getByTestId("depth-badge")).toHaveCount(0);
  // A shallower Depth draws fewer cards, and the frontier nodes it has no card for are counted on Depth.
  await setDepth(page, "Stages");
  await expect.poll(() => cards.count()).toBeLessThan(drawn);
  const left = await frontierCards.evaluateAll((found) => found.map((card) => card.getAttribute("data-node") ?? ""));
  const hidden = frontier.filter((key) => !left.includes(key));
  expect(hidden.length).toBeGreaterThan(0);
  await expect(page.getByTestId("depth-badge")).toHaveText(String(hidden.length));
  await expect(page.getByTestId("depth-button")).toHaveAttribute("title", /that can move now are hidden at this depth; choose Actions or fade the rest to see them/);
  // Fade draws the Actions cards again with those below the depth dimmed, and there is nothing left hidden to count.
  await setFade(page, true);
  await expect.poll(() => cards.count()).toBe(drawn);
  await expect(page.getByTestId("depth-badge")).toHaveCount(0);
  for (const key of hidden) {
    await expect(nodeCard(page, key)).toHaveClass(/node-faded/);
  }
  // With Frontier off, only Depth dims: what Stages draws (the top-level milestone, the top-level stages) is clear, what it left out is not.
  await page.getByTestId("chip-frontier").click();
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(nodeCard(page, "n_kickoff")).not.toHaveClass(/node-faded/);
  await expect(page.locator('[data-testid="node-card"][data-kind="group"]:not([data-parent]).node-faded')).toHaveCount(0);
  for (const key of hidden) {
    await expect(nodeCard(page, key)).toHaveClass(/node-faded/);
  }
});

test("C2, DESIGN opacity: under Fade the cards below the depth are 0.3 whatever their state, a container holding a decision stays clear, and Frontier's dimming joins it without multiplying", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?depth=decisions&fade=1");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  const read = () =>
    page.getByTestId("node-card").evaluateAll((found) =>
      found.map((card) => ({
        key: card.getAttribute("data-node") ?? "",
        kind: card.getAttribute("data-kind") ?? "",
        parent: card.getAttribute("data-parent"),
        done: card.classList.contains("node-done"),
        here: card.getAttribute("data-here") ?? "",
        opacity: Number(getComputedStyle(card).opacity),
      })),
    );
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
});

test("C2, DESIGN opacity: nodes Reveal draws are dimmed by the depth under Fade as the rest are, never lifted by the trace", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?depth=decisions&fade=1");
  await nodeCard(page, "n_partner_runs").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-reveal")).toBeVisible();
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveText("Hide revealed nodes");
  for (const key of ["n_criteria", "n_partner_results"]) {
    await expect(nodeCard(page, key), key).toBeVisible();
    await expect(nodeCard(page, key), key).toHaveCSS("opacity", "0.3");
  }
});
