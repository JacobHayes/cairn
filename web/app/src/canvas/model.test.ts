// The canvas's cards and lines (C1, C2, C4 to C7): what each card says, how each line and
// card looks, what the relevance toggles hide without hiding a blocker, and what the trace
// marks.
import { describe, expect, test } from "vitest";

import { canvasView, actionsAndDecisionsHidden, wholeLevel } from "./canvas.test-support.ts";
import { journeyLooks } from "./journey.ts";
import { DOTTED, cardClasses, hereWords, lineLook } from "./look.ts";
import { DEFAULT_SETTINGS, borderFor, cardsOf, dueTone, linesOf, type CanvasModel, type Card, type Level } from "./model.ts";
import { TRACE_LABELS, traceOverlay } from "./overlay.ts";
import { withRelevanceShown } from "./relevance.ts";

const view = canvasView();
const graph = view.journey.graph.nodes ?? [];

function model(level: Level, ranked = ["n_choose", "n_check"], mine: string[] = []): CanvasModel {
  return { cards: cardsOf(level, graph, journeyLooks(view, { ranked, mine })), lines: linesOf(level, graph) };
}

function card(model: CanvasModel, key: string): Card {
  const found = model.cards.find((each) => each.key === key);
  if (found === undefined) {
    throw new Error(`no card ${key}`);
  }
  return found;
}

const whole = model(wholeLevel());

describe("C1: cards and lines", () => {
  test("a card shows kind, title, state, owner or unassigned, and due", () => {
    const build = card(whole, "n_build");
    expect([build.kind, build.title, build.journey?.state, build.journey?.owner, build.journey?.due?.date]).toEqual([
      "deliverable",
      "Build it",
      "active",
      "Person One",
      "2026-10-01",
    ]);
    expect(card(whole, "n_option").journey?.owner).toBe("unassigned");
    expect(card(whole, "n_stage").journey?.state).toBe("active");
  });

  test("a decision shows its prompt and its answer", () => {
    const choose = card(whole, "n_choose");
    expect(choose.prompt).toBe("Which approach do we take?");
    expect(choose.journey?.answer).toBeUndefined();
  });

  test.each([
    ["explicit", "n_option->n_build", undefined, 0],
    ["condition gate", "n_choose->n_option", DOTTED, 1],
    ["stage opening", "n_kick->n_stage", DOTTED, 1],
  ])("the %s edge %s is drawn with the dash its kind gives, named by its source", (_, id, dash, named) => {
    const line = whole.lines.find((each) => each.id === id);
    expect(line).toBeDefined();
    if (line !== undefined) {
      expect(lineLook(line).dash).toBe(dash);
      expect(line.sources).toHaveLength(named);
    }
  });

  test("an edge standing for an explicit and an implicit edge is solid", () => {
    const [level] = [wholeLevel()];
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
    expect(line && lineLook(line).dash).toBeUndefined();
  });

  test.each([
    ["n_old", "card-not-relevant"],
    ["n_option", "card-undecided"],
  ])("%s is drawn with %s and not hidden by default", (key, look) => {
    expect(cardClasses(card(whole, key))).toContain(look);
    expect(withRelevanceShown(view, whole, DEFAULT_SETTINGS).cards.map((each) => each.key)).toContain(key);
  });
});

describe("C1, C2: hiding by relevance never makes blocked work look free", () => {
  test("hiding undecided nodes marks the work they block", () => {
    const shown = withRelevanceShown(view, whole, { ...DEFAULT_SETTINGS, undecided: false });
    expect(shown.cards.map((each) => each.key)).not.toContain("n_option");
    expect(card(shown, "n_build").hiddenPrerequisites).toEqual(["n_option"]);
    expect(shown.lines.map((line) => line.id)).toEqual(["n_kick->n_stage"]);
  });

  test("hiding not-relevant nodes marks nothing, since they block nothing", () => {
    const shown = withRelevanceShown(view, whole, { ...DEFAULT_SETTINGS, notRelevant: false });
    expect(shown.cards.map((each) => each.key)).not.toContain("n_old");
    expect(shown.cards.flatMap((each) => each.hiddenPrerequisites)).toEqual([]);
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

  test("a container shows its children's badges, most gravity, least slack, and owners", () => {
    expect(card(whole, "n_stage").journey?.badges.map((badge) => badge.flag)).toContain("children active");
    expect(card(whole, "n_build").journey?.children).toEqual({ gravity: 5, slackDays: 40, owners: ["Person One"] });
  });
});

describe("C5: I am here", () => {
  test("the frontier, active work (started early when blocked), and the viewer's items are marked", () => {
    const mine = model(wholeLevel(), [], ["n_option"]);
    expect(hereWords(card(mine, "n_choose"))).toEqual(["actionable now"]);
    expect(hereWords(card(mine, "n_build"))).toEqual(["active, started early"]);
    expect(hereWords(card(mine, "n_option"))).toEqual(["yours"]);
    expect(hereWords(card(mine, "n_kick"))).toEqual([]);
    expect(cardClasses(card(mine, "n_build"))).toContain("card-here");
    expect(cardClasses(card(mine, "n_option"))).toEqual(expect.arrayContaining(["card-mine"]));
    expect(cardClasses(card(mine, "n_option"))).not.toContain("card-here");
  });

  test("only the top-ranked few carry a numbered badge", () => {
    const ranked = ["n_check", "n_choose", "n_build", "n_option", "n_kick", "n_old"];
    const ranks = model(wholeLevel(), ranked).cards.map((each) => [each.key, each.journey?.rank]);
    expect(Object.fromEntries(ranks)).toMatchObject({ n_check: 1, n_choose: 2, n_build: 3, n_option: 4, n_kick: 5, n_old: undefined });
  });
});

describe("C6: priority, quiet", () => {
  test.each([
    ["late", "2026-10-01", true, false, "bad"],
    ["due within a week", "2026-10-09", false, false, "warn"],
    ["due later", "2026-11-30", false, false, "plain"],
    ["finished", "2026-10-01", true, true, "plain"],
  ] as const)("a due date %s is %s", (_, due, overdue, finished, tone) => {
    expect(dueTone(due, "2026-10-06", overdue, finished)).toBe(tone);
  });

  test("gravity drives border weight against the heaviest open node", () => {
    expect([borderFor(0, 8, true), borderFor(4, 8, true), borderFor(8, 8, true), borderFor(8, 8, false)]).toEqual([1, 3, 4, 1]);
    expect(card(whole, "n_kick").journey?.borderPx).toBe(1);
    expect(card(whole, "n_choose").journey?.borderPx).toBe(4);
  });
});

describe("C7: the trace", () => {
  const trace = { node: "n_option", upstream: ["n_choose"], downstream: ["n_build", "n_check", "n_stage"], gravity_contributors: ["n_build"] };

  test("marks upstream, downstream, and gravity contributors, and lights their lines", () => {
    const overlay = traceOverlay(trace, whole, "Optional extra");
    const labels = Object.fromEntries(Object.entries(overlay.marks).map(([key, mark]) => [key, mark.label]));
    expect(labels).toEqual({
      n_option: TRACE_LABELS.traced,
      n_choose: TRACE_LABELS.upstream,
      n_build: TRACE_LABELS.contributor,
      n_check: TRACE_LABELS.downstream,
      n_stage: TRACE_LABELS.downstream,
    });
    expect(overlay.lines).toEqual(["n_choose->n_option", "n_option->n_build"]);
    expect(cardClasses(card(whole, "n_kick"), overlay)).toContain("card-dim");
    expect(overlay.outside).toEqual([]);
  });

  test("a traced node rolled up into a card marks that card; one with no card is listed outside", () => {
    const hidden = model(actionsAndDecisionsHidden());
    const overlay = traceOverlay({ ...trace, node: "n_check", upstream: ["n_choose", "n_option"], downstream: [] }, hidden, "Check the build");
    expect(overlay.marks["n_build"]?.label).toBe(TRACE_LABELS.traced);
    expect(overlay.outside).toEqual(["n_choose"]);
  });
});

/** The canvas journey with `change` made to its derive. */
function changed(change: (view: ReturnType<typeof canvasView>) => void) {
  const edited = canvasView();
  change(edited);
  return edited;
}
const derivedOf = (view: ReturnType<typeof canvasView>, key: string) => {
  const found = view.derived.nodes[key];
  if (found === undefined) {
    throw new Error(`no ${key}`);
  }
  return found;
};
const modelOf = (view: ReturnType<typeof canvasView>, level: Level): CanvasModel => ({
  cards: cardsOf(level, graph, journeyLooks(view, { ranked: [], mine: [] })),
  lines: linesOf(level, graph),
});

describe("C1, C2: what the canvas shows as finished, and markers through roll-ups and ancestors", () => {
  test("a card shows the engine's display state (D8): a skipped node is finished, and checks off in its container's checklist", () => {
    const view = changed((each) => {
      derivedOf(each, "n_build").display_state = "skipped";
      derivedOf(each, "n_check").display_state = "skipped";
    });
    const build = card(modelOf(view, wholeLevel()), "n_build");
    expect([build.journey?.state, build.journey?.finished, hereWords(build)]).toEqual(["skipped", true, []]);
    expect(card(modelOf(view, actionsAndDecisionsHidden()), "n_build").checklist[0]?.done).toBe(true);
  });

  test("a finished group's due date carries no urgency", () => {
    const view = changed((each) => {
      derivedOf(each, "n_stage").dates = { due: { date: "2026-10-07", chain: { constraints: [], fixed: [] } } };
      derivedOf(each, "n_stage").display_state = "done";
    });
    expect(card(modelOf(view, wholeLevel()), "n_stage").journey?.due?.tone).toBe("plain");
  });

  test("hiding undecided nodes marks a container whose rolled-up child they block", () => {
    const view = changed((each) => {
      derivedOf(each, "n_build").blocked_by = [];
      derivedOf(each, "n_check").blocked_by = [{ node: "n_option", via: "explicit" }];
    });
    const shown = withRelevanceShown(view, modelOf(view, actionsAndDecisionsHidden()), { ...DEFAULT_SETTINGS, undecided: false });
    expect(card(shown, "n_build").hiddenPrerequisites).toEqual(["n_option"]);
  });

  test("hiding an undecided container marks what its shown child inherits from it", () => {
    const view = changed((each) => {
      derivedOf(each, "n_stage").relevance = { value: "undecided", decisions: ["n_choose"] };
      derivedOf(each, "n_stage").display_state = "conditional";
      derivedOf(each, "n_stage").blocked_by = [{ node: "n_kick", via: { stage_opening: { group: "n_stage" } } }];
      derivedOf(each, "n_build").blocked_by = [];
      derivedOf(each, "n_build").blocked_through = ["n_stage"];
      const kick = each.journey.graph.state?.nodes?.["n_kick"];
      if (kick !== undefined) {
        kick.state = "pending";
      }
    });
    const shown = withRelevanceShown(view, modelOf(view, wholeLevel()), { ...DEFAULT_SETTINGS, undecided: false });
    expect(shown.cards.map((each) => each.key)).not.toContain("n_stage");
    expect(card(shown, "n_build").parent).toBeUndefined();
    expect(card(shown, "n_build").hiddenPrerequisites).toEqual(["n_kick"]);
  });
});
