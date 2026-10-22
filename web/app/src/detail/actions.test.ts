// The panel's writes: F5's move sent with the rejected edit, D4's bypass offered per node and
// guard and E6's deployment revision named only by a patch that writes an identity.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { violationNode } from "../data/writes.ts";
import { resolvedAnswer } from "./AnswerEditor.tsx";
import { isComplete } from "./contributions.ts";
import { answerInEffect } from "./editors.tsx";
import type { Mutation } from "./model.ts";
import { bypassable, bypassReady, withMove } from "./Rejected.tsx";
import { identityName } from "./sections.tsx";
import { testView } from "./view.test-support.ts";

const violation = (node: string, bypass?: Schema<"Guard">): Schema<"Violation"> => ({
  code: "guard_failed",
  at: { subject: { node } },
  message: "a guard failed",
  ...(bypass === undefined ? {} : { bypassable: bypass }),
});

describe("resolving a rejection", () => {
  it("sends a resolution move after the rejected edit, so the edit is kept", () => {
    const pin: Mutation = { op: "set_pin", node: "n_report", date: "2026-11-25" };
    const move: Mutation = { op: "shift_pin", node: "n_report", offset_days: -5 };
    expect(withMove({ mutations: [pin], base: 4, deployment: 2 }, move)).toEqual([pin, move]);
  });

  it("names the node each violation of a patch over several nodes is on (C9)", () => {
    expect(violationNode(violation("n_final_report"))).toBe("n_final_report");
    expect(violationNode({ code: "duplicate_key", at: {}, message: "no subject" })).toBeUndefined();
  });

  it("takes a reason before finishing past a guard (D4)", () => {
    expect([bypassReady(""), bypassReady("  "), bypassReady("Reviewed out of band")]).toEqual([false, false, true]);
  });

  it("offers a bypass for each node's bypassable guards, each once", () => {
    const offered = bypassable([violation("n_a", "deps_done"), violation("n_a", "deps_done"), violation("n_a", "has_artifact"), violation("n_b")]);
    expect(offered).toEqual([{ node: "n_a", guards: ["deps_done", "has_artifact"] }]);
  });
});

describe("merged identities (E6)", () => {
  const view = testView();
  view.inputs.deployment.aliases = { who_old: "who_one", who_older: "who_old" };

  it("names an answer's identity through the aliases a merge left", () => {
    expect(identityName(view, "who_older")).toBe("Person One");
    expect(identityName(view, "who_two")).toBe("Person Two");
  });

  it("starts an answer editor on the surviving identities, each once", () => {
    expect(resolvedAnswer(view, { identity: "who_old" })).toEqual({ identity: "who_one" });
    expect(resolvedAnswer(view, { identity_list: ["who_older", "who_one", "who_two"] })).toEqual({ identity_list: ["who_one", "who_two"] });
  });
});

describe("answerInEffect (E3)", () => {
  const decided = () => {
    const view = testView();
    const state = view.journey.graph.state ?? {};
    state.nodes = { ...state.nodes, n_when: { state: "decided", provenance: "local" } };
    view.journey.graph.state = state;
    return view;
  };

  it("is the answer of a decided, relevant decision", () => {
    expect(answerInEffect(decided(), "n_when")).toEqual({ date: "2026-11-20" });
  });

  it("is nothing once the decision is out of scope or open, though the answer is kept", () => {
    const view = decided();
    const when = view.derived.nodes["n_when"];
    if (when !== undefined) {
      when.relevance = { value: "not_relevant" };
    }
    expect(answerInEffect(view, "n_when")).toBeUndefined();
    expect(answerInEffect(testView(), "n_when")).toBeUndefined();
  });
});

describe("contributor lists", () => {
  it("are complete when they hold their total, and capped otherwise", () => {
    const entry = { node: "n_a", score: 1 };
    expect(isComplete({ entries: [entry], total: 1 })).toBe(true);
    expect(isComplete({ entries: [entry], total: 60 })).toBe(false);
  });
});
