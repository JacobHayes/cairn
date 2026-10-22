// C1: the date relationships between cards. Where the engine's level puts them (between stand-ins)
// is the engine's; here: which card each is drawn between when an end is rolled up or hidden, what
// is dropped, how parts between the same cards merge, and that the card at
// the far end of the selected node's line stays in focus.
import { describe, expect, test } from "vitest";

import { canvasView, actionsAndDecisionsHidden, wholeLevel } from "./canvas.test-support.ts";
import { datesAround, dateLinesOf, mergeDateParts } from "./dates.ts";
import { cardsOf, type CanvasModel, type DateLine, type DateLink, type Level, type LevelDateLink } from "./model.ts";
import { TRACE_LABELS, traceOverlay } from "./overlay.ts";

const graph = canvasView().journey.graph.nodes ?? [];

const rule = (node: string, source: string, direction: "before" | "after" = "before", days = 3): DateLink => ({ node, source, kind: "rule", rule: "due_by", direction, days });
const link = (from: string, to: string, orders: boolean, ...links: DateLink[]): LevelDateLink => ({ from, to, orders, links });
const withLinks = (level: Level, ...dates: LevelDateLink[]): Level => ({ ...level, date_links: dates });
const model = (level: Level): CanvasModel & { dates: DateLine[] } => ({ cards: cardsOf(level, graph), lines: [], dates: dateLinesOf(level, graph) });
const ends = (found: { dates: DateLine[] }) => found.dates.map((line) => [line.from, line.to, line.orders]);

describe("dateLinesOf (C1)", () => {
  test("a relationship is drawn between the cards its ends stand in, wherever they are rolled up", () => {
    // The check is rolled up into the build at this level, and the engine's link names its stand-in.
    const level = withLinks(actionsAndDecisionsHidden(), link("n_build", "n_kick", true, rule("n_check", "n_kick")));
    const found = model(level);
    expect(ends(found)).toEqual([["n_build", "n_kick", true]]);
    expect(found.dates[0]?.entries.map((each) => each.link.node)).toEqual(["n_check"]);
  });

  test("a relationship whose end has no card, or that lands on one card or inside one, is not drawn", () => {
    const level = withLinks(
      wholeLevel(),
      link("n_kick", "n_gone", true, rule("n_gone", "n_kick")),
      link("n_build", "n_build", true, rule("n_build", "n_build")),
      link("n_build", "n_check", true, rule("n_check", "n_build")),
      link("n_kick", "n_stage", true, rule("n_stage", "n_kick")),
    );
    expect(ends(model(level))).toEqual([["n_kick", "n_stage", true]]);
  });

  test("relationships between the same two cards are one line, which fixes the order when any does", () => {
    const level = withLinks(wholeLevel(), link("n_kick", "n_stage", true, rule("n_stage", "n_kick")), link("n_stage", "n_kick", false, rule("n_kick", "n_stage", "after", 0)));
    const [line, ...rest] = model(level).dates;
    expect(rest).toEqual([]);
    expect([line?.from, line?.to, line?.orders, line?.entries.length]).toEqual(["n_kick", "n_stage", true, 2]);
    // A relationship that fixes no order also joins a line of its own pair when no other is there.
    expect(ends(model(withLinks(wholeLevel(), link("n_kick", "n_stage", false, rule("n_stage", "n_kick", "after")))))).toEqual([["n_kick", "n_stage", false]]);
  });

  test("a line says its first two relationships and counts the rest", () => {
    const level = withLinks(wholeLevel(), link("n_kick", "n_stage", true, rule("n_stage", "n_kick", "before", 1), rule("n_stage", "n_kick", "before", 2), rule("n_stage", "n_kick", "before", 3), rule("n_stage", "n_kick", "before", 4)));
    const [line] = model(level).dates;
    expect(line?.sentence.split("; ")).toHaveLength(3);
    expect(line?.sentence).toContain("and 2 more");
  });
});

describe("date lines around a selection", () => {
  test("a selected card has the lines at either end, and none selected has none", () => {
    const level = withLinks(wholeLevel(), link("n_kick", "n_stage", true, rule("n_stage", "n_kick")), link("n_stage", "n_option", true, rule("n_stage", "n_option")));
    const { dates } = model(level);
    expect(datesAround(dates, "n_stage").map((line) => line.id)).toEqual(dates.map((line) => line.id));
    expect(datesAround(dates, "n_kick").map((line) => [line.from, line.to])).toEqual([["n_kick", "n_stage"]]);
    expect(datesAround(dates, undefined)).toEqual([]);
  });

  test("the far end of a selected node's line is not faded and hangs no tag; a trace mark it already has stays", () => {
    const found = model(wholeLevel());
    const trace = { node: "n_build", upstream: ["n_option"], downstream: [], gravity_contributors: [] };
    const overlay = traceOverlay(trace, found, "Build it", new Set(["n_kick", "n_option"]));
    expect(overlay.marks["n_kick"]).toMatchObject({ quiet: true });
    expect(overlay.marks["n_option"]?.label).toBe(TRACE_LABELS.needs);
    expect(overlay.marks["n_old"]).toBeUndefined();
    expect(overlay.dim).toBe(true);
  });
});

describe("a container around a date line's far end", () => {
  test("stays in focus with it, and one that holds nothing dated is left to the trace", () => {
    const found = model(wholeLevel());
    const inside = { ...found, cards: found.cards.map((card) => (card.key === "n_kick" ? { ...card, parent: "n_old" } : card)) };
    const trace = { node: "n_build", upstream: [], downstream: [], gravity_contributors: [] };
    const overlay = traceOverlay(trace, inside, "Build it", new Set(["n_kick"]));
    expect(overlay.marks["n_old"]).toMatchObject({ quiet: true });
    expect(traceOverlay(trace, inside, "Build it").marks["n_old"]).toBeUndefined();
  });
});

describe("mergeDateParts", () => {
  test("keeps the direction of a part that fixes the order, whichever comes first", () => {
    const entry = { link: rule("n_b", "n_a"), words: "b" };
    const lines = mergeDateParts([
      { from: "n_b", to: "n_a", orders: false, entries: [entry] },
      { from: "n_a", to: "n_b", orders: true, entries: [entry] },
    ]);
    expect(lines.map((line) => [line.from, line.to, line.orders, line.entries.length])).toEqual([["n_a", "n_b", true, 2]]);
  });
});
