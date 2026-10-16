// C5, C7: out of focus is one level, and the lowest opacity wins: a trace never lifts a card that
// Frontier or Decisions left unlit, and an unlit card keeps its trace mark.
import { expect, test } from "vitest";

import { canvasView, wholeLevel } from "./canvas.test-support.ts";
import { fadedCards, nodeClasses } from "./look.ts";
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

test("a card out of Frontier's focus or inside Depth's fade is dimmed, once; a container holding a lit card stays clear", () => {
  const cards = cardsOf(wholeLevel(), canvasView().journey.graph.nodes ?? []);
  const keys = cards.map((card) => card.key);
  // The fixture draws n_check inside n_build inside n_stage.
  expect(cards.find((card) => card.key === "n_check")?.parent).toBe("n_build");
  expect(cards.find((card) => card.key === "n_build")?.parent).toBe("n_stage");
  expect(fadedCards(cards, undefined, undefined).size).toBe(0);
  // Depth's fade alone: only the cards it names.
  expect([...fadedCards(cards, new Set(["n_check"]), undefined)]).toEqual(["n_check"]);
  // Frontier alone: everything but the lit card and the containers above it.
  const lit = fadedCards(cards, undefined, new Set(["n_check"]));
  expect(keys.filter((key) => !lit.has(key)).sort()).toEqual(["n_build", "n_check", "n_stage"]);
  // Both: the lit card that Depth dims is dimmed, its containers stay clear, and nothing is dimmed twice over.
  const both = fadedCards(cards, new Set(["n_check"]), new Set(["n_check"]));
  expect(both).toEqual(new Set([...lit, "n_check"]));
  expect(both.has("n_build")).toBe(false);
});
