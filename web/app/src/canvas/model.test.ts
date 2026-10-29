// The canvas's cards and lines (C1, C2, C4 to C7): what each card says in its one body and foot
// line, how each line and card looks, what the level request asks for, and what the trace marks
// and the Signals lens puts on a card.
import { describe, expect, test } from "vitest";

import { canvasView, actionsAndDecisionsHidden, wholeLevel } from "./canvas.test-support.ts";
import { journeyLooks } from "./journey.ts";
import { dueWords } from "../status/when.ts";
import { lensChips } from "./lens.ts";
import { DOTTED, hereWords, lineLook, nodeClasses } from "./look.ts";
import { cardsOf, linesOf, type CanvasModel, type Card, type Level } from "./model.ts";
import { TRACE_LABELS, traceOverlay } from "./overlay.ts";

const view = canvasView();
const graph = view.journey.graph.nodes ?? [];

function model(level: Level, ranked = ["n_choose", "n_check"], mine: string[] = [], ready = view): CanvasModel {
  const ranking = { ...ready, derived: { ...ready.derived, acting_frontier: ranked } };
  const looks = journeyLooks(ranking, { mine, owned: mine });
  return { cards: cardsOf(level, graph, looks), lines: linesOf(level, graph, (edge) => looks.waiting(edge)) };
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
    expect(card(mine, "n_choose").journey?.foot).toEqual({ words: dueWords("2026-10-09", "2026-10-06", true), tone: "warn" });
    expect(card(mine, "n_choose").journey?.owner).toEqual({ words: "You", missing: false });
    // Owned by someone else: the owner is not on the card.
    expect(card(whole, "n_build").journey?.owner).toBeUndefined();
    expect(card(whole, "n_build").journey?.foot).toEqual({ words: dueWords("2026-10-01", "2026-10-06"), tone: "bad" });
    const unowned = changed((each) => {
      derivedOf(each, "n_check").unassigned = true;
    });
    expect(card(model(wholeLevel(), [], [], unowned), "n_check").journey?.owner).toMatchObject({ missing: true });
  });

  test("a card has at most one body line: a decided decision's answer, a container's progress, or what a conditional node depends on", () => {
    const decided = changed((each) => {
      derivedOf(each, "n_choose").display_state = "done";
      each.journey.graph.state = { ...each.journey.graph.state, answers: { n_choose: { boolean: true } }, rationales: { n_choose: "It is the cheaper one.\n\nAnd the quicker." } };
    });
    expect(card(model(wholeLevel(), [], [], decided), "n_choose").journey?.body).toEqual({ kind: "answer", text: "yes", rationale: "It is the cheaper one." });
    expect(card(whole, "n_choose").journey?.body).toBeUndefined();
    expect(card(whole, "n_stage").journey?.body).toEqual({ kind: "progress", done: 0, total: 2, badge: undefined });
    expect(card(whole, "n_option").journey?.body).toEqual({ kind: "depends", text: "If \"Approach\" is answered" });
    expect(card(whole, "n_old").journey?.body).toBeUndefined();
  });

  test("work that holds no other nodes shows how far its checklist is ticked; with children its children's progress wins; with no items there is no line", () => {
    const listed = changed((each) => {
      derivedOf(each, "n_check").checklist_count = { ticked: 1, total: 3 };
      derivedOf(each, "n_build").checklist_count = { ticked: 2, total: 4 };
      derivedOf(each, "n_old").checklist_count = { ticked: 0, total: 0 };
    });
    const cards = model(wholeLevel(), [], [], listed);
    expect(card(cards, "n_check").journey?.body).toEqual({ kind: "checklist", ticked: 1, total: 3 });
    // n_build holds n_check: its checklist is the inspector's, its card keeps its children's progress.
    expect(card(cards, "n_build").journey?.body).toMatchObject({ kind: "progress" });
    expect(card(cards, "n_old").journey?.body).toBeUndefined();
  });

  test("a node snoozed through its container says so in its foot, and one snoozed itself says until when", () => {
    const snoozed = changed((each) => {
      derivedOf(each, "n_check").snoozed_via = "n_stage";
      derivedOf(each, "n_build").display_state = "snoozed";
      derivedOf(each, "n_build").snoozed = { date: "2026-10-20" };
    });
    const cards = model(wholeLevel(), [], [], snoozed);
    expect(card(cards, "n_check").journey?.foot).toEqual({ words: "Via Build stage", title: "Snoozed through Build stage", tone: "plain" });
    expect(card(cards, "n_build").journey?.foot).toEqual({ words: "Until Oct 20", title: "Snoozed until Oct 20", tone: "plain" });
  });

  test("a node snoozed until another is done keeps 'is done' apart from the name, which is what a short foot cuts", () => {
    const snoozed = changed((each) => {
      derivedOf(each, "n_build").display_state = "snoozed";
      derivedOf(each, "n_build").snoozed = { node: "n_stage" };
    });
    expect(card(model(wholeLevel(), [], [], snoozed), "n_build").journey?.foot).toEqual({ words: "Until Build stage", tail: " is done", title: "Snoozed until Build stage is done", tone: "plain" });
  });

  test("a container carries one roll-up badge, the most urgent", () => {
    const level = wholeLevel();
    level.nodes = level.nodes.map((node) => (node.key === "n_stage" ? { ...node, roll_up: { all_blocked: true, progress: { done: 0, total: 1, overdue: 0, decisions_needed: 0 } } } : node));
    expect(card(model(level), "n_stage").journey?.body).toMatchObject({ badge: { flag: "all blocked" } });
    const deciding = changed((each) => {
      derivedOf(each, "n_stage").progress = { done: 0, total: 1, overdue: 0, decisions_needed: 1 };
    });
    expect(card(model(level, undefined, [], deciding), "n_stage").journey?.body).toMatchObject({ badge: { flag: "decision to make" } });
  });

});

describe("a route's canvas (8.11)", () => {
  test("draws cards with no journey state, by the same level", () => {
    expect(cardsOf(wholeLevel(), graph).map((each) => each.journey)).toEqual(whole.cards.map(() => undefined));
  });
});

describe("C1: lines", () => {
  test.each([
    ["n_option->n_build", "requires", undefined, "arrow", "n_option", "n_build"],
    ["n_choose->n_option", "condition", DOTTED, "diamond", "n_choose", "n_option"],
    ["n_kick->n_stage", "stage_opening", DOTTED, "bar", "n_kick", "n_stage"],
  ])("the edge %s is a %s: dash %s, a %s marker, and its one edge has %s as its requirement and %s as its dependent", (id, kind, dash, marker, requirement, dependent) => {
    const line = whole.lines.find((each) => each.id === id);
    expect(line).toBeDefined();
    if (line !== undefined) {
      expect([line.kind, lineLook(line).dash, lineLook(line).marker]).toEqual([kind, dash, marker]);
      expect(line.edges.map((each) => [each.requirement, each.dependent, each.kind])).toEqual([[requirement, dependent, kind]]);
    }
  });

  test("an edge with a hidden end is drawn to the card that stands for it; one inside a card, or into nowhere, is not drawn, and edges between the same two cards are one", () => {
    const level = actionsAndDecisionsHidden();
    const edge = (from: string, to: string) => ({ from, to, implicit: false, underlying: [{ requirement: from, dependent: to, origin: "explicit" as const }] });
    level.edges = [
      edge("n_option", "n_build"),
      // The check is rolled up into the build: this joins it to the same line as the build's own.
      edge("n_option", "n_check"),
      // A card's requirement on the node rolled up inside it, and a container's on its own child.
      edge("n_build", "n_check"),
      edge("n_stage", "n_build"),
      // The decision is hidden and nothing drawn holds it.
      edge("n_choose", "n_option"),
    ];
    // The line stands for the two edges that landed on it, each still between its own ends.
    expect(linesOf(level, graph).map((line) => [line.id, line.edges.map((each) => [each.requirement, each.dependent])])).toEqual([["n_option->n_build", [["n_option", "n_build"], ["n_option", "n_check"]]]]);
  });

  test("a stand-in for a requirement and a condition is solid and says how many it stands for", () => {
    const level = wholeLevel();
    level.edges = [
      {
        from: "n_choose",
        to: "n_option",
        implicit: false,
        underlying: [
          { requirement: "n_choose", dependent: "n_option", origin: "explicit" },
          { requirement: "n_choose", dependent: "n_option", origin: "condition" },
        ],
      },
    ];
    const [line] = linesOf(level, graph);
    expect([line?.kind, line?.count]).toEqual(["requires", 2]);
    expect(line && lineLook(line).dash).toBeUndefined();
    // The requirement is listed first, the condition after it.
    expect(line?.edges.map((each) => each.kind)).toEqual(["requires", "condition"]);
  });

});

describe("C1: what a line waits on and holds", () => {
  test("an edge waits by the dependent's own unsatisfied gates, not by how its requirement looks", () => {
    const waits = (ready: ReturnType<typeof canvasView>, id: string) => model(wholeLevel(), undefined, [], ready).lines.find((each) => each.id === id)?.edges.map((each) => each.waits);
    // The fixture: Build is blocked by Optional extra (explicit), Optional extra by Approach (its condition).
    expect([waits(view, "n_option->n_build"), waits(view, "n_choose->n_option"), waits(view, "n_kick->n_stage")]).toEqual([[true], [true], [false]]);
    // The requirement is finished, but the engine still lists it as holding the dependent (a skipped container with kept work pending does this).
    const kept = changed((each) => { derivedOf(each, "n_option").display_state = "done"; });
    expect(waits(kept, "n_option->n_build")).toEqual([true]);
    // The requirement is unfinished, but the engine no longer lists it (a not-relevant prerequisite stops gating).
    const dropped = changed((each) => { derivedOf(each, "n_build").blocked_by = []; });
    expect(waits(dropped, "n_option->n_build")).toEqual([false]);
    // A condition waits only on a condition blocker of this dependent; the same decision blocking as an explicit requirement is another edge.
    const other = changed((each) => { derivedOf(each, "n_option").blocked_by = [{ node: "n_choose", via: { condition: { condition_on: "n_build" } } }]; });
    expect(waits(other, "n_choose->n_option")).toEqual([false]);
    // A stage opening waits only on a blocker that names that stage.
    const opening = changed((each) => { derivedOf(each, "n_stage").blocked_by = [{ node: "n_kick", via: { stage_opening: { group: "n_stage" } } }]; });
    const elsewhere = changed((each) => { derivedOf(each, "n_stage").blocked_by = [{ node: "n_kick", via: { stage_opening: { group: "n_other" } } }]; });
    expect([waits(opening, "n_kick->n_stage"), waits(elsewhere, "n_kick->n_stage")]).toEqual([[true], [false]]);
  });

  test("a line is satisfied exactly when no edge it stands for waits", () => {
    const lines = model(wholeLevel(), undefined, [], view).lines;
    const satisfied = (id: string) => lines.find((each) => each.id === id)?.satisfied;
    expect([satisfied("n_kick->n_stage"), satisfied("n_option->n_build")]).toEqual([true, false]);
  });

  test("a route's lines carry no state", () => {
    const [line] = linesOf(wholeLevel(), graph);
    expect([line?.edges.map((each) => each.waits), line?.satisfied]).toEqual([[undefined], undefined]);
  });

  test("a requirement on a container holds what is inside it; one on a leaf holds nothing", () => {
    const level = wholeLevel();
    level.edges = [
      { from: "n_kick", to: "n_stage", underlying: [{ requirement: "n_kick", dependent: "n_stage", origin: "explicit" }] },
      { from: "n_kick", to: "n_choose", underlying: [{ requirement: "n_kick", dependent: "n_choose", origin: "explicit" }] },
    ];
    expect(linesOf(level, graph).map((line) => [line.id, line.edges[0]?.holds])).toEqual([["n_kick->n_stage", true], ["n_kick->n_choose", false]]);
  });

});

describe("C2, C4: roll-ups", () => {
  const hidden = model(actionsAndDecisionsHidden());

  test("a node whose prerequisite has no visible stand-in carries the marker", () => {
    expect(card(hidden, "n_option").hiddenPrerequisites).toEqual(["n_choose"]);
  });

  test("a card shows the engine's display state (D8): a skipped node is finished", () => {
    const skipped = changed((each) => {
      derivedOf(each, "n_build").display_state = "skipped";
      derivedOf(each, "n_check").display_state = "skipped";
    });
    const build = card(model(wholeLevel(), [], [], skipped), "n_build");
    expect([build.journey?.state, build.journey?.finished, hereWords(build)]).toEqual(["skipped", true, []]);
    expect(nodeClasses(build)).toContain("node-skipped");
  });

  test("done is filled whatever the focus: state classes are independent of the focus class", () => {
    const finished = changed((each) => { derivedOf(each, "n_build").display_state = "done"; });
    const done = card(model(wholeLevel(), [], [], finished), "n_build");
    expect(nodeClasses(done, { faded: true })).toEqual(expect.arrayContaining(["node-done", "node-faded"]));
    expect(nodeClasses(done)).not.toContain("node-skipped");
  });
});

describe("C5: I am here, and the rank tags", () => {
  test("the frontier, active work, and the viewer's items are marked", () => {
    const mine = model(wholeLevel(), [], ["n_option"]);
    expect(hereWords(card(mine, "n_choose"))).toEqual(["actionable now"]);
    expect(hereWords(card(mine, "n_build"))).toEqual(["active"]);
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

  test("a selected container's members are traced like any other card: tagged, with the lines between them lit", () => {
    const overlay = traceOverlay({ ...trace, node: "n_stage", upstream: ["n_build", "n_check", "n_option"], downstream: [] }, whole, "Build stage");
    expect(["n_build", "n_check", "n_option"].map((key) => overlay.marks[key]?.label)).toEqual([TRACE_LABELS.needs, TRACE_LABELS.needs, TRACE_LABELS.needs]);
    expect(overlay.marks["n_stage"]?.quiet).toBe(true);
    expect(overlay.lines).toEqual({ "n_option->n_build": "ink" });
  });

  test("a requirement into a container that holds something needed is upstream, and one out of a container that holds something unblocked is downstream", () => {
    const line = whole.lines[0];
    if (line === undefined) {
      throw new Error("the fixture has no line");
    }
    const lines = [...whole.lines, { ...line, id: "n_stage->n_choose", from: "n_stage", to: "n_choose", satisfied: false }];
    const overlay = (traced: typeof trace) => traceOverlay(traced, { ...whole, lines }, "Build it").lines;
    expect(overlay({ ...trace, node: "n_option", upstream: ["n_kick", "n_build"], downstream: [] })["n_kick->n_stage"]).toBe("ink");
    expect(overlay({ ...trace, node: "n_option", upstream: [], downstream: ["n_build", "n_choose"] })["n_stage->n_choose"]).toBe("accent");
  });

  test("a finished upstream line is lit like any other", () => {
    const finished = changed((each) => {
      derivedOf(each, "n_choose").display_state = "done";
    });
    expect(traceOverlay(trace, model(wholeLevel(), [], [], finished), "Optional extra").lines["n_choose->n_option"]).toBe("ink");
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
  test("each lens puts one labelled number on the cards that have it; a container shows the gravity of its whole area; only the acting frontier has Unlocks", () => {
    const ranked = model(wholeLevel(), ["n_check", "n_choose"]);
    const text = (lens: Parameters<typeof lensChips>[1]) => Object.fromEntries([...lensChips(ranked.cards, lens)].map(([key, chip]) => [key, chip.text]));
    expect(text("rank")).toEqual({ n_check: "Rank #1", n_choose: "Rank #2" });
    expect(text("gravity")).toMatchObject({ n_choose: "Gravity 8", n_option: "Gravity 4.5", n_build: "Gravity 9" });
    expect(text("gravity")["n_kick"]).toBeUndefined();
    expect(Object.keys(text("unlocks")).sort()).toEqual(["n_choose"]);
    expect(text("slack")).toEqual({ n_choose: "Slack 2 days", n_build: "Slack 40 days" });
  });
});

describe("C2: what the rest of a card says", () => {
  test("a container with a child needing breakdown says so in its badge", () => {
    const breakdown = wholeLevel();
    breakdown.nodes = breakdown.nodes.map((node) => (node.key === "n_stage" ? { ...node, roll_up: { needs_breakdown: true, progress: { done: 0, total: 1, overdue: 0, decisions_needed: 0 } } } : node));
    expect(card(model(breakdown), "n_stage").journey?.body).toMatchObject({ badge: { flag: "needs breakdown" } });
  });

  test("an overdue open node says how late it is in its foot, and a node that is not open does not", () => {
    const late = (open: boolean) =>
      changed((each) => {
        const derived = derivedOf(each, "n_build");
        derived.open = open;
        derived.dates = { ...derived.dates, overdue_days: 3 };
      });
    expect(card(model(wholeLevel(), [], [], late(true)), "n_build").journey?.foot).toMatchObject({ words: "3 days late", tone: "bad" });
    expect(card(model(wholeLevel(), [], [], late(false)), "n_build").journey?.foot).toBeUndefined();
  });
});

describe("only a top-level group is a stage, whose zoomed-out head carries the state chip", () => {
  test("a group nested inside another group, however deep, is not a stage; the group at the top is", () => {
    const nodes = [
      { key: "n_outer", id: "outer", kind: "group", title: "Outer" },
      { key: "n_middle", id: "middle", kind: "group", title: "Middle", parent: "n_outer" },
      { key: "n_inner", id: "inner", kind: "group", title: "Inner", parent: "n_middle" },
      { key: "n_work", id: "work", kind: "action", title: "Work", parent: "n_inner" },
    ] as typeof graph;
    const level: Level = { nodes: nodes.map((node) => ({ key: node.key, parent: node.parent })) } as Level;
    expect(cardsOf(level, nodes).map((each) => [each.key, each.stage])).toEqual([
      ["n_outer", true],
      ["n_middle", false],
      ["n_inner", false],
      ["n_work", false],
    ]);
  });
});
