// Proposal review's pure model: the diff a proposal makes (C14), the gate on applying it
// (I6: a stale or re-drafted proposal is reviewed again before it applies), and the
// reviewer's edits to its items and mutations.
import { describe, expect, it } from "vitest";

import type { Graph, Mutation } from "../authoring/graph.ts";
import {
  applyBlockers,
  attemptFor,
  DIFF_LABELS,
  DIFF_NOTES,
  diffMarks,
  filterCounts,
  filterOf,
  reviewEntries,
  graphDiff,
  intervening,
  carryIds,
  withFieldValue,
  removedChoices,
  reviewedAfter,
  unionGraph,
  unresolvedHere,
  withExclusion,
  withMapping,
  withMutation,
  withMutationsAdded,
  withOrphanKept,
  withResolution,
  withoutMutation,
  type GateInput,
  type ProposalDraft,
  type ReviewEvent,
} from "./model.ts";

const before: Graph = {
  nodes: [
    { key: "n_a", id: "a", kind: "deliverable", title: "A" },
    { key: "n_b", id: "b", kind: "action", title: "B", parent: "n_a" },
    { key: "n_c", id: "c", kind: "action", title: "C", requires: ["n_b"] },
  ],
  retired_keys: {},
};

const after: Graph = {
  nodes: [
    { key: "n_a", id: "a", kind: "deliverable", title: "A, renamed" },
    { key: "n_c", id: "c", kind: "action", title: "C" },
    { key: "n_d", id: "d", kind: "action", title: "D", parent: "n_a", requires: ["n_a"] },
  ],
  retired_keys: { nodes: ["n_b"] },
};

describe("graphDiff (C14)", () => {
  it("names what was added, removed, and changed, field by field and edge by edge", () => {
    expect(graphDiff(before, after)).toEqual({
      added: ["n_d"],
      removed: ["n_b"],
      changed: { n_a: ["title"], n_c: ["requires"] },
      edgesAdded: [{ node: "n_d", requires: "n_a" }],
      edgesRemoved: [{ node: "n_c", requires: "n_b" }],
    });
  });

  it("is empty between a graph and itself", () => {
    const diff = graphDiff(before, before);
    expect([diff.added, diff.removed, diff.edgesAdded, diff.edgesRemoved, Object.keys(diff.changed)]).toEqual([[], [], [], [], []]);
  });

  it("from nothing, adds every node of a graph a proposal creates", () => {
    expect(graphDiff(undefined, after).added).toEqual(["n_a", "n_c", "n_d"]);
  });
});

describe("filterCounts (C14)", () => {
  it("counts an edge as an addition or a removal, never as a change", () => {
    const diff = graphDiff(before, after);
    expect(filterCounts(diffMarks(diff, []), diff)).toEqual({ all: 6, conflicts: 0, add: 2, change: 2, remove: 2 });
  });
});

describe("unionGraph (C14)", () => {
  it("draws a removed node back in place, without the keys its removal retired or any state", () => {
    const union = unionGraph(before, { ...after, state: { nodes: {} } });
    expect(union.nodes?.map((node) => [node.key, node.parent ?? null])).toEqual([
      ["n_a", null],
      ["n_c", null],
      ["n_d", "n_a"],
      ["n_b", "n_a"],
    ]);
    expect([union.retired_keys, union.state]).toEqual([undefined, undefined]);
  });
});

const open: GateInput = {
  proposal: { status: "open", revision: 3 },
  reviewed: 3,
  dirty: false,
  editing: false,
  stale: false,
  preview: { unresolved: [], violations: [] },
};

describe("applyBlockers (C14, I6)", () => {
  const cases: [string, Partial<GateInput>, string[]][] = [
    ["nothing, once reviewed at its revision", {}, []],
    ["an unsaved edit", { dirty: true }, ["unsaved"]],
    ["an editor holding a value with a problem", { editing: true }, ["editing"]],
    ["a destination that moved", { stale: true }, ["stale"]],
    ["an item still needing a choice", { preview: { unresolved: [{ item: 0, reason: "no_choice" }], violations: [] } }, ["unresolved"]],
    ["a violation of the candidate", { preview: { unresolved: [], violations: [{ code: "containment_cycle", at: {}, message: "" }] } }, ["invalid"]],
    ["no preview yet", { preview: undefined }, ["no_preview"]],
    ["a review of an older revision", { reviewed: 2 }, ["unreviewed"]],
    ["a proposal no longer open, whatever else", { proposal: { status: "applied", revision: 3 }, dirty: true }, ["not_open"]],
  ];
  it.each(cases)("blocks on %s", (_, change, blockers) => {
    expect(applyBlockers({ ...open, ...change })).toEqual(blockers);
  });
});

describe("reviewedAfter (I6)", () => {
  const confirm = (events: ReviewEvent[]) => events.reduce<number | undefined>(reviewedAfter, undefined);

  it("confirms the revision a reviewer marks", () => {
    expect(confirm([{ event: "marked", revision: 3 }])).toBe(3);
  });

  it("asks for a renewed review after a refresh: the refreshed proposal does not apply until marked again", () => {
    const reviewed = confirm([
      { event: "marked", revision: 3 },
      { event: "refreshed", proposal: { revision: 4 } },
    ]);
    expect(applyBlockers({ ...open, proposal: { status: "open", revision: 4 }, reviewed })).toEqual(["unreviewed"]);
    expect(applyBlockers({ ...open, proposal: { status: "open", revision: 4 }, reviewed: reviewedAfter(reviewed, { event: "marked", revision: 4 }) })).toEqual([]);
  });

  it("asks for a renewed review after the proposal is saved changed", () => {
    const reviewed = confirm([
      { event: "marked", revision: 3 },
      { event: "saved", proposal: { revision: 4 } },
    ]);
    expect(applyBlockers({ ...open, proposal: { status: "open", revision: 4 }, reviewed })).toEqual(["unreviewed"]);
  });
});

describe("intervening (I6)", () => {
  it("is the last patches of the history, one per revision the destination moved", () => {
    expect(intervening(["p1", "p2", "p3", "p4"], 5, 7)).toEqual(["p3", "p4"]);
    expect(intervening(["p1"], 5, 5)).toEqual([]);
  });
});

describe("removedChoices (B7)", () => {
  it("names the choices an answer holds that the route no longer has", () => {
    expect(removedChoices({ multi_choice: ["a", "b", "c"] }, ["a", { id: "c", title: "C" }])).toEqual(["b"]);
    expect(removedChoices({ single_choice: "a" }, ["a"])).toEqual([]);
    expect(removedChoices({ boolean: true }, [])).toEqual([]);
  });
});

describe("a reviewer's edits (C14)", () => {
  const draft: ProposalDraft = {
    title: "Upgrade",
    destination_revision: 7,
    items: [
      { item: "conflict", conflict: { about: "default_owner", journey: "r_a", route: "r_b" } },
      { item: "orphan", node: "n_b", keep: true, removal: { node: "n_b" } },
      { item: "participation", entity: "e_a", uses: [{ node: "n_a", kind: "k_owner" }] },
      { item: "exclusion", node: "n_c", excluded: false },
    ],
    mutations: [{ op: "upgrade", to: 2 }, { op: "clear_pin", node: "n_a" }],
  };

  it("each edits its own item and leaves the rest", () => {
    const edited = withExclusion(withMapping(withOrphanKept(withResolution(draft, 0, "take_route"), 1, false), 2, "drop"), 3, true);
    expect(edited.items).toEqual([
      { ...draft.items?.[0], resolution: "take_route" },
      { ...draft.items?.[1], keep: false },
      { ...draft.items?.[2], mapping: "drop" },
      { ...draft.items?.[3], excluded: true },
    ]);
    expect(edited.mutations).toEqual(draft.mutations);
  });

  it("an edit for another kind of item changes nothing", () => {
    expect(withOrphanKept(draft, 0, false)).toEqual(draft);
  });

  it("drops, replaces, and adds mutations in place", () => {
    expect(withoutMutation(draft, 0).mutations).toEqual([{ op: "clear_pin", node: "n_a" }]);
    expect(withMutation(draft, 1, { op: "set_pin", node: "n_a", date: "2026-11-02" }).mutations?.[1]).toEqual({ op: "set_pin", node: "n_a", date: "2026-11-02" });
    expect(withMutationsAdded(draft, [{ op: "clear_pin", node: "n_c" }]).mutations).toHaveLength(3);
  });
});

describe("diffMarks (C14)", () => {
  it("marks the diff, and lets the items' conflicts, orphans, and exclusions say more", () => {
    const diff = graphDiff(before, after);
    const marks = diffMarks(diff, [
      { item: "conflict", conflict: { about: "field", node: "n_c", journey: { title: "C" }, route: { title: "See" } } },
      { item: "orphan", node: "n_b", keep: true, removal: { node: "n_b" } },
    ]);
    expect(Object.fromEntries(Object.entries(marks).map(([key, mark]) => [key, [mark.label, mark.note, filterOf(mark)]]))).toEqual({
      n_a: [DIFF_LABELS.changed, undefined, "change"],
      n_b: [DIFF_LABELS.changed, DIFF_NOTES.orphan, "change"],
      n_c: [DIFF_LABELS.changed, DIFF_NOTES.conflict, "conflicts"],
      n_d: [DIFF_LABELS.added, undefined, "add"],
    });
  });
});

describe("reviewEntries (C14)", () => {
  it("lists a removal that cascades from a removed container beneath it, and the rest by kind of change", () => {
    const nodes = (keys: [string, string?][]) => ({ nodes: keys.map(([key, parent]) => ({ key, id: key, kind: "action" as const, title: key, ...(parent === undefined ? {} : { parent }) })) });
    const was = nodes([["a"], ["b", "a"], ["c", "b"], ["d"], ["e"]]);
    const now = nodes([["d"], ["e"], ["f"]]);
    const diff = graphDiff(was, now);
    const entries = reviewEntries(diff, diffMarks(diff, []), was);
    expect(entries.map((entry) => [entry.key, entry.cause])).toEqual([
      ["f", undefined],
      ["a", undefined],
      ["b", "a"],
      ["c", "a"],
    ]);
  });
});

describe("unresolvedHere (C14)", () => {
  it("finds a conflict with no choice or one it does not offer, and an unmapped entity", () => {
    const conflict = { about: "default_owner", journey: "r_a", route: "r_b" } as const;
    const found = unresolvedHere([
      { item: "conflict", conflict },
      { item: "conflict", conflict, resolution: "reopen" },
      { item: "conflict", conflict, resolution: "take_route" },
      { item: "participation", entity: "e_a", uses: [] },
      { item: "participation", entity: "e_b", uses: [], mapping: "drop" },
      { item: "orphan", node: "n_a", keep: true, removal: { node: "n_a" } },
    ]);
    expect([...found]).toEqual([
      [0, "no_choice"],
      [1, "not_offered"],
      [3, "no_choice"],
    ]);
  });
});

describe("carryIds (C14)", () => {
  const field = (title: string): Mutation => ({ op: "set_node_field", node: "n_a", value: { title } });
  const list = [field("one"), field("two"), field("three")];
  let count = 0;
  const fresh = () => {
    count += 1;
    return `id-${String(count)}`;
  };
  const named = carryIds({ mutations: [], ids: [] }, list, fresh);

  it("keeps every name when a mutation is edited in place", () => {
    const edited = [field("one"), field("two, edited"), field("three")];
    expect(carryIds({ mutations: list, ids: named }, edited, fresh)).toEqual(named);
  });

  it("drops only the dropped mutation's name, even among mutations alike", () => {
    expect(carryIds({ mutations: list, ids: named }, [field("one"), field("three")], fresh)).toEqual([named[0], named[2]]);
  });

  it("names added mutations afresh and keeps the rest", () => {
    const more = carryIds({ mutations: list, ids: named }, [...list, field("four")], fresh);
    expect(more.slice(0, 3)).toEqual(named);
    expect(named).not.toContain(more[3]);
  });
});

describe("withFieldValue", () => {
  it("leaves a cleared field out rather than writing null", () => {
    const node = withFieldValue({ key: "n_a", id: "a", kind: "action", title: "A", estimate: 2 }, { estimate: null });
    expect("estimate" in node).toBe(false);
  });
});

describe("attemptFor (I6, H5)", () => {
  const fresh = () => ({ id: "pr_new", patchId: "p_new" });
  const lost = { asked: "upgrade to 2", id: "pr_lost", patchId: "p_lost" };

  it("sends a draft whose answer never came again under the same ids", () => {
    expect(attemptFor(lost, "upgrade to 2", fresh)).toEqual(lost);
  });

  it("sends anything else under fresh ids", () => {
    expect(attemptFor(lost, "upgrade to 3", fresh)).toEqual({ asked: "upgrade to 3", id: "pr_new", patchId: "p_new" });
    expect(attemptFor(undefined, "upgrade to 2", fresh)).toEqual({ asked: "upgrade to 2", id: "pr_new", patchId: "p_new" });
  });
});
