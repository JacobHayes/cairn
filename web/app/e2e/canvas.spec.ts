// The journey canvas over the vendor evaluation (C2 to C7): the flow of reading the graph in a
// real browser, which only a browser can check (SVG hit-testing, the wheel and its modifiers,
// the viewport). It opens fitted at every detail, expands in place, traces a selection, follows an
// edge, and pans and zooms. What the canvas draws (its levels, lines, lens, layout) is the
// model's tests'.
import { expect, test } from "@playwright/test";

import { lineBetween, pointOn, viewportOf, viewportSettled } from "./canvas.ts";
import { nodeCard, nodePanel, openJourney, setDetail } from "./shell.ts";

test("C2 to C7: the graph opens fitted at every detail, expands in place, traces a selection, follows an edge, and pans and zooms", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  // It opens at the stock view: every detail, fitted to the whole graph, as Fit does.
  await expect(page.getByTestId("detail-button")).toHaveAccessibleName("Detail: All");
  // At Stages the stage holding the work is open and says so, the others fold.
  await setDetail(page, "Stages");
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
  // At far zoom a card's glyph, not its colour, names its state: no two states share one.
  const glyphs = await page.getByTestId("card-glyph").evaluateAll((all) => all.map((each) => [each.getAttribute("data-state"), each.textContent]));
  expect(new Set(glyphs.map(([, glyph]) => glyph)).size).toBe(new Set(glyphs.map(([state]) => state)).size);
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

test("the controls row: Detail redraws, Filter wears a badge for what it hides, Frontier only highlights", async ({ page }) => {
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
  await expect(page.getByTestId("detail-badge")).toHaveCount(0);
  // A coarser Detail draws fewer cards, and the highlighted nodes it has no card for are counted on Detail.
  await page.getByTestId("chip-frontier").click();
  await page.getByTestId("chip-decisions").click();
  await setDetail(page, "Stages");
  await expect.poll(() => cards.count()).toBeLessThan(drawn);
  await expect(page.getByTestId("detail-badge")).toBeVisible();
  await expect(page.getByTestId("detail-button")).toHaveAttribute("title", /hidden at this detail; choose All to see them/);
});
