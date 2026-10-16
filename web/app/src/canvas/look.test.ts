// C5, C7: out of focus is one level, and the lowest opacity wins: a trace never lifts a card that
// Frontier or Decisions left unlit, and an unlit card keeps its trace mark.
import { expect, test } from "vitest";

import { canvasView, wholeLevel } from "./canvas.test-support.ts";
import { nodeClasses } from "./look.ts";
import { cardsOf } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

test("a traced card off the lit set is faded and keeps its mark; a lit one is not faded", () => {
  const [card] = cardsOf(wholeLevel(), canvasView().journey.graph.nodes ?? []);
  if (card === undefined) {
    throw new Error("the fixture's level draws a card");
  }
  const overlay: CanvasOverlay = { title: "Trace", marks: { [card.key]: { tone: "accent", label: "needs" } }, lines: {}, dim: true, outside: [] };
  const unlit = nodeClasses(card, { overlay, faded: true });
  expect(unlit).toContain("node-faded");
  expect(unlit).toContain("node-marked");
  expect(nodeClasses(card, { overlay, faded: false })).not.toContain("node-faded");
});
