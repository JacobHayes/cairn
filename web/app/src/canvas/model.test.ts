// The canvas's cards and lines (C1, C2, C4 to C7): what each card says in its one body and foot
// line, how each line and card looks, what the level request asks for, and what the trace marks
// and the Signals lens puts on a card.
import { describe, expect, test } from "vitest";

import { canvasView, actionsAndDecisionsHidden, wholeLevel } from "./canvas.test-support.ts";
import { journeyLooks } from "./journey.ts";
import { lensChips } from "./lens.ts";
import { DOTTED, hereWords, lineLook } from "./look.ts";
import { cardsOf, linesOf, type CanvasModel, type Card, type Level } from "./model.ts";
import { TRACE_LABELS, traceOverlay } from "./overlay.ts";
import { DEFAULT_VIEW, levelRequest } from "./settings.ts";

const view = canvasView();
const graph = view.journey.graph.nodes ?? [];

function model(level: Level, ranked = ["n_choose", "n_check"], mine: string[] = [], ready = view): CanvasModel {
  const looks = journeyLooks(ready, { ranked, mine, owned: mine });
  return { cards: cardsOf(level, graph, looks), lines: linesOf(level, graph, (key) => looks.finished(key)) };
}

function card(model: CanvasModel, key: string): Card {
  const found = model.cards.find((each) => each.key === key);
  if (found === undefined) {
    throw new Error(`no card ${key}`);
  }
  return found;
}

/** The canvas journey with `change` made to its derive and document. */
function changed(change: (view: ReturnType<typeof canvasView>) => void) {
  const edited = canvasView();
  change(edited);
  return edited;
}
const derivedOf = (ready: ReturnType<typeof canvasView>, key: string) => {
  const found = ready.derived.nodes[key];
  if (found === undefined) {
    throw new Error(`no ${key}`);
  }
  return found;
};

const whole = model(wholeLevel());

describe("C1: what a card says", () => {
  test("a card's foot gives the one date in words, and its owner only when it is the viewer's or missing", () => {
    const mine = model(wholeLevel(), [], ["n_choose"]);
    expect(card(mine, "n_choose").journey?.foot).toEqual({ words: "Due in 3 days", tone: "warn" });
    expect(card(mine, "n_choose").journey?.owner).toEqual({ words: "You", missing: false });
    // Owned by someone else: the owner is not on the card.
    expect(card(whole, "n_build").journey?.owner).toBeUndefined();
    expect(card(whole, "n_build").journey?.foot).toEqual({ words: "5 days late", tone: "bad" });
    const unowned = changed((each) => {
      derivedOf(each, "n_check").unassigned = true;
    });
    expect(card(model(wholeLevel(), [], [], unowned), "n_check").journey?.owner).toEqual({ words: "Unassigned", missing: true });
  });

  test("a card has at most one body line: a decided decision's answer, a container's progress, or what a conditional node depends on", () => {
    const decided = changed((each) => {
      derivedOf(each, "n_choose").display_state = "done";
      each.journey.graph.state = { ...each.journey.graph.state, answers: { n_choose: { boolean: true } }, rationales: { n_choose: "It is the cheaper one.\n\nAnd the quicker." } };
    });
    expect(card(model(wholeLevel(), [], [], decided), "n_choose").journey?.body).toEqual({ kind: "answer", text: "yes", rationale: "It is the cheaper one." });
    expect(card(whole, "n_choose").journey?.body).toBeUndefined();
    expect(card(whole, "n_stage").journey?.body).toEqual({ kind: "progress", done: 0, total: 2, badge: undefined });
    expect(card(whole, "n_option").journey?.body).toEqual({ kind: "depends", text: "If Approach" });
    expect(card(whole, "n_old").journey?.body).toBeUndefined();
  });

  test("a node snoozed through its container says so in its foot, and one snoozed itself says until when", () => {
    const snoozed = changed((each) => {
      derivedOf(each, "n_check").snoozed_via = "n_stage";
      derivedOf(each, "n_build").display_state = "snoozed";
      derivedOf(each, "n_build").snoozed = { date: "2026-10-20" };
    });
    const cards = model(wholeLevel(), [], [], snoozed);
    expect(card(cards, "n_check").journey?.foot).toEqual({ words: "z via Build stage", tone: "plain" });
    expect(card(cards, "n_build").journey?.foot).toEqual({ words: "Snoozed until Oct 20", tone: "plain" });
  });

  test("a container carries one roll-up badge, the most urgent", () => {
    const level = wholeLevel();
    level.nodes = level.nodes.map((node) => (node.key === "n_stage" ? { ...node, roll_up: { all_blocked: true, decision_needed: true } } : node));
    expect(card(model(level), "n_stage").journey?.body).toMatchObject({ badge: { flag: "decision to make" } });
  });
});

describe("C1: lines", () => {
  test.each([
    ["n_option->n_build", "requires", undefined, "arrow", "Build it needs Optional extra"],
    ["n_choose->n_option", "condition", DOTTED, "diamond", "Applies only depending on Approach"],
    ["n_kick->n_stage", "stage_opening", DOTTED, "bar", "Build stage waits for Kickoff, where it opens"],
  ])("the edge %s is a %s: dash %s, a %s marker, and it says %s", (id, kind, dash, marker, sentence) => {
    const line = whole.lines.find((each) => each.id === id);
    expect(line).toBeDefined();
    if (line !== undefined) {
      expect([line.kind, lineLook(line).dash, lineLook(line).marker, line.sentence]).toEqual([kind, dash, marker, sentence]);
    }
  });

  test("a stand-in for a requirement and a condition is solid and says how many it stands for", () => {
    const level = wholeLevel();
    level.edges = [
      {
        from: "n_choose",
        to: "n_option",
        gates: true,
        implicit: false,
        underlying: [
          { requirement: "n_choose", dependent: "n_option", origin: "explicit", gates: true },
          { requirement: "n_choose", dependent: "n_option", origin: "condition", gates: true },
        ],
      },
    ];
    const [line] = linesOf(level, graph);
    expect([line?.kind, line?.count]).toEqual(["requires", 2]);
    expect(line && lineLook(line).dash).toBeUndefined();
  });

});

describe("C1, C2: the level request", () => {
  test.each([
    ["conditional left out", { undecided: false }, ["relevant"]],
    ["not relevant shown", { notRelevant: true }, undefined],
    ["the defaults hide only the settled not-relevant", {}, ["relevant", "conditional"]],
  ])("%s", (_, settings, display) => {
    expect(levelRequest({ ...DEFAULT_VIEW, ...settings }, { kinds: ["group"], collapsed: [] }).display).toEqual(display);
  });

});

describe("C2, C4: roll-ups", () => {
  const hidden = model(actionsAndDecisionsHidden());

  test("a hidden action is a checklist item on its deliverable, finished or not", () => {
    expect(card(hidden, "n_build").checklist).toEqual([{ key: "n_check", title: "Check the build", kind: "action", done: false }]);
  });

  test("a node whose prerequisite has no visible stand-in carries the marker", () => {
    expect(card(hidden, "n_option").hiddenPrerequisites).toEqual(["n_choose"]);
  });

  test("a card shows the engine's display state (D8): a skipped node is finished, and checks off in its container's checklist", () => {
    const skipped = changed((each) => {
      derivedOf(each, "n_build").display_state = "skipped";
      derivedOf(each, "n_check").display_state = "skipped";
    });
    const build = card(model(wholeLevel(), [], [], skipped), "n_build");
    expect([build.journey?.state, build.journey?.finished, hereWords(build)]).toEqual(["skipped", true, []]);
    expect(card(model(actionsAndDecisionsHidden(), [], [], skipped), "n_build").checklist[0]?.done).toBe(true);
  });

  test("a node of a shown kind that rolled up is not a checklist item", () => {
    const level = actionsAndDecisionsHidden();
    level.shown = [...level.shown, "action"];
    expect(card(model(level), "n_build").checklist).toEqual([]);
  });
});

describe("C5: I am here, and the rank tags", () => {
  test("the frontier, active work (started early when blocked), and the viewer's items are marked", () => {
    const mine = model(wholeLevel(), [], ["n_option"]);
    expect(hereWords(card(mine, "n_choose"))).toEqual(["actionable now"]);
    expect(hereWords(card(mine, "n_build"))).toEqual(["active, started early"]);
    expect(hereWords(card(mine, "n_option"))).toEqual(["yours"]);
    expect(hereWords(card(mine, "n_kick"))).toEqual([]);
  });

  test("a card knows its place in the rank order, for the lens; the canvas hangs the tag on the top three only", () => {
    const ranked = ["n_check", "n_choose", "n_build", "n_option", "n_kick", "n_old"];
    const ranks = model(wholeLevel(), ranked).cards.map((each) => [each.key, each.journey?.rank]);
    expect(Object.fromEntries(ranks)).toMatchObject({ n_check: 1, n_choose: 2, n_build: 3, n_option: 4, n_kick: 5, n_old: 6 });
  });
});

describe("C7: the trace of a selected node", () => {
  const trace = { node: "n_option", upstream: ["n_choose"], downstream: ["n_build", "n_check", "n_stage"], gravity_contributors: ["n_build"] };

  test("tags what it needs and what it unblocks, dots its gravity contributors, and lights the lines in ink and the accent", () => {
    const overlay = traceOverlay(trace, whole, "Optional extra");
    const labels = Object.fromEntries(Object.entries(overlay.marks).map(([key, mark]) => [key, mark.label]));
    expect(labels).toEqual({
      n_option: TRACE_LABELS.traced,
      n_choose: TRACE_LABELS.needs,
      n_build: TRACE_LABELS.unblocks,
      n_check: TRACE_LABELS.unblocks,
      n_stage: TRACE_LABELS.unblocks,
    });
    expect(overlay.marks["n_build"]?.contributor).toBe(true);
    expect(overlay.marks["n_check"]?.contributor).toBeUndefined();
    expect(overlay.lines).toEqual({ "n_choose->n_option": "ink", "n_option->n_build": "accent" });
    expect(overlay.outside).toEqual([]);
  });

  test("a selected container does not tag its own members, and keeps the lines from them to cards outside it", () => {
    const overlay = traceOverlay({ ...trace, node: "n_stage", upstream: ["n_build", "n_check", "n_option"], downstream: [] }, whole, "Build stage");
    const quiet = (key: string) => overlay.marks[key]?.quiet;
    expect([quiet("n_build"), quiet("n_check"), overlay.marks["n_option"]?.label]).toEqual([true, true, TRACE_LABELS.needs]);
    expect(overlay.lines).toEqual({ "n_option->n_build": "ink" });
  });

  test("a finished upstream line is faint", () => {
    const finished = changed((each) => {
      derivedOf(each, "n_choose").display_state = "done";
    });
    expect(traceOverlay(trace, model(wholeLevel(), [], [], finished), "Optional extra").lines["n_choose->n_option"]).toBe("faint");
  });

  test("a traced node rolled up into a card marks that card; one with no card is listed outside", () => {
    const hidden = model(actionsAndDecisionsHidden());
    const overlay = traceOverlay({ ...trace, node: "n_check", upstream: ["n_choose", "n_option"], downstream: [] }, hidden, "Check the build");
    expect(overlay.marks["n_build"]?.label).toBe(TRACE_LABELS.traced);
    expect(overlay.outside).toEqual(["n_choose"]);
  });

  test("a node rolled up into a card for its relevance class, not its kind, still marks that card", () => {
    const level = actionsAndDecisionsHidden();
    level.shown = [...level.shown, "action"];
    const overlay = traceOverlay({ ...trace, node: "n_check", upstream: [], downstream: [] }, model(level), "Check the build");
    expect(overlay.marks["n_build"]?.label).toBe(TRACE_LABELS.traced);
  });
});

describe("C6: the Signals lens", () => {
  test("each lens puts one labelled number on the cards that have it; a container shows the gravity of its whole area; only the acting frontier has Unblocks", () => {
    const ranked = model(wholeLevel(), ["n_check", "n_choose"]);
    const text = (lens: Parameters<typeof lensChips>[1]) => Object.fromEntries([...lensChips(ranked.cards, lens)].map(([key, chip]) => [key, chip.text]));
    expect(text("rank")).toEqual({ n_check: "Rank #1", n_choose: "Rank #2" });
    expect(text("gravity")).toMatchObject({ n_choose: "Gravity 8", n_option: "Gravity 4.5", n_build: "Gravity 9" });
    expect(text("gravity")["n_kick"]).toBeUndefined();
    expect(Object.keys(text("unblocks")).sort()).toEqual(["n_check", "n_choose"]);
    expect(text("slack")).toEqual({ n_choose: "Slack 2 days", n_build: "Slack 40 days" });
  });
});
