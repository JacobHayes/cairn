// The journey canvas over the vendor evaluation (C2 to C7): the flow of reading the graph in a
// real browser, which only a browser can check (SVG hit-testing, the wheel and its modifiers,
// the viewport). It opens at its current stage, expands in place, traces a selection, follows an
// edge, and pans and zooms. What the canvas draws (its levels, lines, lens, layout) is the
// model's tests'.
import { expect, test } from "@playwright/test";

import { lineBetween, pointOn, viewportOf } from "./canvas.ts";
import { nodeCard, nodePanel, openJourney } from "./shell.ts";

test("C2 to C7: the graph opens at its current stage, expands in place, traces a selection, follows an edge, and pans and zooms", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  // It opens at Stages: the stage holding the work is open and says so, the others fold.
  await expect(page.getByTestId("ladder").getByLabel("Stages")).toBeChecked();
  await expect(nodeCard(page, "n_reporting").getByTestId("card-current")).toBeVisible();
  // It opens where the work is, at a size titles can be read at; the Fit button shows the rest.
  expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(0.6);
  await page.keyboard.press("f");
  await expect(nodeCard(page, "n_setup").getByTestId("card-expand")).toBeVisible();
  await expect(page.locator(".react-flow__attribution")).toHaveCount(0);
  // A folded stage traces what the nodes folded into it trace: the line leaving it is followed.
  await nodeCard(page, "n_setup").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-bar")).not.toContainText("unblocks 0");
  // Expanding opens it in place, and the address remembers.
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await expect(page).toHaveURL(/open=n_setup/);
  // At far zoom a card's glyph, not its colour, names its state: no two states share one.
  const glyphs = await page.getByTestId("card-glyph").evaluateAll((all) => all.map((each) => [each.getAttribute("data-state"), each.textContent]));
  expect(new Set(glyphs.map(([, glyph]) => glyph)).size).toBe(new Set(glyphs.map(([state]) => state)).size);
  // Selecting a node traces it: what it needs and what it unblocks are tagged, and the bar says how many.
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-bar")).toContainText("Test plan");
  await expect(page.getByTestId("card-mark").filter({ hasText: "needs" }).first()).toBeVisible();
  await expect(page.getByTestId("card-mark").filter({ hasText: "unblocks" }).first()).toBeVisible();
  // What the step folds away is counted, and Reveal draws it.
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveCount(0);
  // Hovering an edge says what it is; clicking it opens its card, which follows it to either end.
  const edge = lineBetween(page, "n_access", "n_plan");
  const at = await pointOn(edge);
  await page.mouse.move(at.x, at.y);
  await expect(page.getByTestId("edge-hover")).toContainText("Test plan needs Environment access");
  await page.mouse.click(at.x, at.y);
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await page.getByRole("link", { name: "Go to Environment access" }).click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  // A wheel pans and never zooms; with the control key it zooms. A shift-drag outside the Select mode draws no lasso.
  const [start, pointer] = [await viewportOf(page), { x: 600, y: 500 }];
  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.wheel(0, 120);
  await expect.poll(async () => (await viewportOf(page)).y).not.toBe(start.y);
  expect((await viewportOf(page)).zoom).toBe(start.zoom);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");
  await expect.poll(async () => (await viewportOf(page)).zoom).not.toBe(start.zoom);
  await page.keyboard.down("Shift");
  await page.mouse.down();
  await page.mouse.move(pointer.x + 120, pointer.y + 80);
  await expect(page.locator(".react-flow__selection")).toHaveCount(0);
  await page.mouse.up();
  await page.keyboard.up("Shift");
});
