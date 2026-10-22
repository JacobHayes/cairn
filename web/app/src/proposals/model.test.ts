// Proposal review's pure model: the diff a proposal makes (C14), the gate on applying it
// (I6: a stale or re-drafted proposal is reviewed again before it applies), and the
// reviewer's edits to its items and mutations.
import { describe, expect, it } from "vitest";

import type { Graph, Mutation } from "../authoring/graph.ts";
import {
  applyBlockers,
  DIFF_NOTES,
  diffMarks,
  reviewEntries,
  graphDiff,
  intervening,
  carryIds,
  needsChoice,
  withFieldValue,
  withExclusion,
  withMapping,
  withMutation,
  withMutationsAdded,
  withOrphanKept,
  withResolution,
  withoutMutation,
  type GateInput,
  type ProposalDraft,
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

const open: GateInput = {
  proposal: { status: "open", revision: 3 },
  dirty: false,
  editing: false,
  stale: false,
  preview: { unresolved: [], violations: [] },
};

describe("applyBlockers (C14, I6)", () => {
  const cases: [string, Partial<GateInput>, string[]][] = [
    ["nothing", {}, []],
    ["an unsaved edit", { dirty: true }, ["unsaved"]],
    ["an editor holding a value with a problem", { editing: true }, ["editing"]],
    ["a destination that moved", { stale: true }, ["stale"]],
    ["an item still needing a choice", { preview: { unresolved: [{ item: 0, reason: "no_choice" }], violations: [] } }, ["unresolved"]],
    ["a violation of the candidate", { preview: { unresolved: [], violations: [{ code: "containment_cycle", at: {}, message: "" }] } }, ["invalid"]],
    ["no preview yet", { preview: undefined }, ["no_preview"]],
    ["a proposal no longer open, whatever else", { proposal: { status: "applied", revision: 3 }, dirty: true }, ["not_open"]],
  ];
  it.each(cases)("blocks on %s", (_, change, blockers) => {
    expect(applyBlockers({ ...open, ...change })).toEqual(blockers);
  });
});

describe("intervening (I6)", () => {
  it("is the last patches of the history, one per revision the destination moved", () => {
    expect(intervening(["p1", "p2", "p3", "p4"], 5, 7)).toEqual(["p3", "p4"]);
    expect(intervening(["p1"], 5, 5)).toEqual([]);
  });
});


describe("a reviewer's edits (C14)", () => {
  const draft: ProposalDraft = {
    title: "Upgrade",
    destination_revision: 7,
    items: [
      { item: "conflict", conflict: { about: "default_owner", journey: "r_a", route: "r_b" } },
      { item: "orphan", node: "n_b", keep: true, removal: { node: "n_b" } },
      { item: "participation", identity: "who_a", uses: [{ node: "n_a", kind: "k_owner" }] },
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
  const items = [
    { item: "conflict" as const, conflict: { about: "field" as const, node: "n_c", journey: { title: "C" }, route: { title: "See" } } },
    { item: "orphan" as const, node: "n_b", keep: true, removal: { node: "n_b" } },
  ];

  it("marks the diff by kind, and lets the items' orphans say more", () => {
    const marks = diffMarks(graphDiff(before, after), items, new Set());
    expect(Object.fromEntries(Object.entries(marks).map(([key, mark]) => [key, [mark.kind, mark.note, mark.choice]]))).toEqual({
      n_a: ["changed", undefined, undefined],
      n_b: ["changed", DIFF_NOTES.orphan, undefined],
      n_c: ["changed", undefined, undefined],
      n_d: ["added", undefined, undefined],
    });
  });

  it("asks for a choice only while its conflict is unresolved", () => {
    const diff = graphDiff(before, after);
    expect(diffMarks(diff, items, new Set([0]))["n_c"]).toMatchObject({ kind: "changed", choice: true });
    expect(diffMarks(diff, items, new Set())["n_c"]?.choice).toBeUndefined();
  });

  it("marks a node whose state a proposal changes though its definition is the same", () => {
    const diff = graphDiff({ ...before, state: { nodes: { n_a: { state: "todo", provenance: "local" } } } }, { ...before, state: { nodes: { n_a: { state: "done", provenance: "local" } } } });
    expect(diffMarks(diff, [], new Set())["n_a"]).toMatchObject({ kind: "changed" });
  });
});

describe("reviewEntries (C14)", () => {
  it("lists a removal that cascades from a removed container beneath it, and the rest by kind of change", () => {
    const nodes = (keys: [string, string?][]) => ({ nodes: keys.map(([key, parent]) => ({ key, id: key, kind: "action" as const, title: key, ...(parent === undefined ? {} : { parent }) })) });
    const was = nodes([["a"], ["b", "a"], ["c", "b"], ["d"], ["e"]]);
    const now = nodes([["d"], ["e"], ["f"]]);
    const diff = graphDiff(was, now);
    const entries = reviewEntries(diff, diffMarks(diff, [], new Set()), was);
    expect(entries.map((entry) => [entry.key, entry.cause])).toEqual([
      ["f", undefined],
      ["a", undefined],
      ["b", "a"],
      ["c", "a"],
    ]);
  });
});

describe("needsChoice (C14)", () => {
  it("is true for the items a reviewer resolves and false for those only shown", () => {
    expect(needsChoice({ item: "conflict", conflict: { about: "default_owner", journey: "r_a", route: "r_b" } })).toBe(true);
    expect(needsChoice({ item: "participation", identity: "who_a", uses: [] })).toBe(true);
    expect(needsChoice({ item: "orphan", node: "n_a", keep: true, removal: { node: "n_a" } })).toBe(false);
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
