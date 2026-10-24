// C8's node detail composed in the tab from the document and its derive, and D1's moves.
import { describe, expect, it } from "vitest";

import { checklistOf, failingGuards, flagsOf, guardBypass, itemTitle, milestonesFirst, movesFrom, nodeDetail, rankPlaces, transition, waitsMore, waitsOn, type Move, type NodeKind, type State } from "./model.ts";
import { ITEM_GONE } from "../ui/gone.ts";
import { checklistView, testView } from "./view.test-support.ts";

describe("nodeDetail", () => {
  it("composes a node's path, record, pin, annotations, and local edits from the document", () => {
    const detail = nodeDetail(testView(), "n_report");
    expect(detail?.path).toBe("stage/report");
    expect(detail?.ancestors.map((node) => node.key)).toEqual(["n_stage"]);
    expect(detail?.record).toMatchObject({ state: "active", provenance: "from_route" });
    expect(detail?.pin).toBe("2026-11-02");
    expect(detail?.annotations.map((annotation) => annotation.body.key)).toEqual(["a_link"]);
    expect(detail?.localEdits).toEqual([{ field: "title" }]);
    expect(detail?.fedBy).toBeUndefined();
  });

  it("lists a container's children with their states, initial where none is stored", () => {
    const children = nodeDetail(testView(), "n_stage")?.children;
    expect(children?.map((child) => [child.key, child.state])).toEqual([
      ["n_report", "active"],
      ["n_findings", "todo"],
    ]);
  });

  it("names the decision that feeds a milestone's pin (E3)", () => {
    expect(nodeDetail(testView(), "n_meeting")?.fedBy?.key).toBe("n_when");
  });

  it("is undefined for a node the journey does not hold", () => {
    expect(nodeDetail(testView(), "n_absent")).toBeUndefined();
  });
});

describe("a node's checklist (G5)", () => {
  it("lists the items in checklist order and marks the ticked ones; a node with none has an empty list", () => {
    expect(checklistOf(checklistView(), "n_findings").map((item) => [item.id, item.ticked])).toEqual([["a", false], ["b", true], ["c", false]]);
    expect(checklistOf(checklistView(), "n_report")).toEqual([]);
  });

  it("names an item by its title, and one that is no longer listed in words", () => {
    expect(itemTitle(checklistView(), "n_findings", "b")).toBe("Support team briefed");
    expect(itemTitle(checklistView(), "n_findings", "removed")).toBe(ITEM_GONE);
  });
});

describe("movesFrom (D1)", () => {
  const cases: [NodeKind, State, Move[]][] = [
    ["deliverable", "todo", ["start", "complete", "skip"]],
    ["action", "active", ["stop", "complete", "skip"]],
    ["deliverable", "done", ["reopen"]],
    ["decision", "open", ["skip"]],
    ["decision", "decided", ["reopen"]],
    ["milestone", "pending", ["complete", "skip"]],
    ["milestone", "done", ["reopen"]],
    ["group", "derived", ["skip", "mark_done"]],
    ["group", "skipped", ["reopen"]],
    ["group", "done", ["reopen"]],
  ];
  it.each(cases)("a %s in %s can %j", (kind, state, moves) => {
    expect(movesFrom(kind, state)).toEqual(moves);
  });

  it("carries a skip's reason in its mutation", () => {
    expect(transition("n_a", "skip", "out of scope")).toEqual({
      op: "transition",
      node: "n_a",
      transition: { skip: { reason: "out of scope" } },
    });
    expect(transition("n_a", "complete")).toEqual({ op: "transition", node: "n_a", transition: "complete" });
  });
});

describe("a guard bypass (D4)", () => {
  it("leaves the reason out when none, or only spaces, was given, and trims one that was", () => {
    const bypass = (reason?: string) => guardBypass("n_a", ["deps_done"], reason);
    const guard = (reason?: string) => (bypass(reason) as { override: { guard_bypass: object } }).override.guard_bypass;
    expect(guard()).toEqual({ guards: ["deps_done"] });
    expect(guard("   ")).toEqual({ guards: ["deps_done"] });
    expect(guard(" Ready sooner ")).toEqual({ guards: ["deps_done"], reason: "Ready sooner" });
  });
});

describe("flags (D3)", () => {
  it("flags only real attention: started early is said by the sentence, never flagged", () => {
    const view = testView();
    const flags = (key: string) => {
      const derived = view.derived.nodes[key];
      if (derived === undefined) {
        throw new Error(`no ${key}`);
      }
      return flagsOf(derived).map((each) => each.flag);
    };
    expect(view.derived.nodes["n_report"]?.started_early).toBe(true);
    expect(flags("n_report")).toEqual([]);
    const overdue = view.derived.nodes["n_report"];
    if (overdue !== undefined) {
      view.derived.nodes["n_report"] = { ...overdue, overdue: true };
    }
    expect(flags("n_report")).toEqual(["overdue"]);
    expect(flags("n_findings")).toEqual([]);
    expect(flags("n_stage")).toEqual([]);
  });
});

describe("what the engine derived, read as it is", () => {
  it("waitsOn names each dependency with the ancestor holding it, and nothing for a node that waits on none", () => {
    const view = testView();
    expect(waitsOn(view, "n_report")).toEqual([{ node: "n_findings", via: "explicit", holder: undefined }]);
    expect(waitsOn(view, "n_meeting")).toEqual([]);
  });

  it("waitsMore counts what the engine cut from the list and says nothing when it listed everything", () => {
    const view = testView();
    const derived = view.derived.nodes["n_report"];
    if (derived === undefined) {
      throw new Error("the test journey has n_report");
    }
    view.derived.nodes["n_report"] = { ...derived, waits_on: { entries: derived.waits_on.entries, total: derived.waits_on.entries.length + 3 } };
    expect(waitsMore(view, "n_report")).toBe(3);
    expect(waitsMore(view, "n_meeting")).toBe(0);
  });

  it("failingGuards is the engine's list, empty when it names none", () => {
    const view = testView();
    const derived = view.derived.nodes["n_report"];
    expect(derived && failingGuards({ ...derived, failing_guards: ["has_note"] })).toEqual(["has_note"]);
    expect(derived && failingGuards({ ...derived, failing_guards: [] })).toEqual([]);
  });

  it("rankPlaces numbers the acting frontier in the engine's order from one, and leaves other nodes out", () => {
    const view = testView();
    view.derived.acting_frontier = ["n_meeting", "n_findings"];
    expect([...rankPlaces(view)]).toEqual([["n_meeting", 1], ["n_findings", 2]]);
    const reordered = { ...view, derived: { ...view.derived, acting_frontier: ["n_findings", "n_meeting"] } };
    expect([...rankPlaces(reordered)]).toEqual([["n_findings", 1], ["n_meeting", 2]]);
  });

  it("milestonesFirst puts milestones ahead, then orders by key", () => {
    const view = testView();
    expect(milestonesFirst(view, ["n_report", "n_meeting", "n_findings"], (key) => key)).toEqual(["n_meeting", "n_findings", "n_report"]);
  });
});
