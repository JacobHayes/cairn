// The panel's writes: F5's move sent with the rejected edit, D4's bypass offered per node and
// guard and E6's deployment revision named only by a patch that writes an identity.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { violationNode } from "../data/writes.ts";
import { resolvedAnswer } from "./AnswerEditor.tsx";
import { isComplete } from "./contributions.ts";
import type { Mutation } from "./model.ts";
import { bypassable, untickedIn, withBypass, withMove, withTicks } from "./Rejected.tsx";
import { identityName } from "./sections.tsx";
import { testView } from "./view.test-support.ts";

const violation = (node: string, failure?: Schema<"GuardFailure">): Schema<"Violation"> => ({
  code: "guard_failed",
  at: { subject: { node } },
  message: "a guard failed",
  ...(failure === undefined ? {} : { failures: [failure] }),
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

  it("completes every node a refusal names anyway at once, on one reason, leaving other moves alone (D4)", () => {
    const skip: Mutation = { op: "transition", node: "n_c", transition: { skip: { reason: "No" } } };
    const attempt: Mutation[] = [{ op: "transition", node: "n_a", transition: "complete" }, { op: "transition", node: "n_b", transition: "complete" }, skip];
    const sent = withBypass({ mutations: attempt, base: 4, deployment: 2 }, ["n_a"], "Reviewed");
    expect(sent).toEqual([{ op: "transition", node: "n_a", transition: { complete_anyway: { reason: "Reviewed" } } }, attempt[1], skip]);
  });

  it("sends the completion without a reason when none was typed (D4)", () => {
    const completes: Mutation[] = [{ op: "transition", node: "n_a", transition: "complete" }];
    expect(withBypass({ mutations: completes, base: 4, deployment: 2 }, ["n_a"], "  ")).toEqual([{ op: "transition", node: "n_a", transition: { complete_anyway: {} } }]);
  });

  it("offers a bypass for each node with a failed guard, once", () => {
    const offered = bypassable([violation("n_a", "missing_note"), violation("n_a", "missing_note"), violation("n_a", "missing_artifact"), violation("n_b")]);
    expect(offered).toEqual(["n_a"]);
  });
});

describe("a refusal for unticked checklist items (G5, D4)", () => {
  const unticked = (node: string, item: string): Schema<"Violation"> => violation(node, { unticked: item });

  it("groups the unticked items per node, and says ticking is all it needs only when nothing else broke", () => {
    const refused = [unticked("n_a", "x"), unticked("n_a", "y"), unticked("n_b", "z")];
    expect(untickedIn(refused)).toEqual({ byNode: [{ node: "n_a", items: ["x", "y"] }, { node: "n_b", items: ["z"] }], only: true });
    expect(untickedIn([...refused, violation("n_a", "missing_note")]).only).toBe(false);
    expect(untickedIn([violation("n_a", "missing_note")])).toEqual({ byNode: [], only: false });
  });

  it("sends the ticks before the refused patch, so the guard reads them ticked", () => {
    const complete: Mutation = { op: "transition", node: "n_a", transition: "complete" };
    const sent = withTicks({ mutations: [complete], base: 4, deployment: 2 }, [{ node: "n_a", items: ["x", "y"] }]);
    expect(sent).toEqual([{ op: "set_ticked", node: "n_a", item: "x", ticked: true }, { op: "set_ticked", node: "n_a", item: "y", ticked: true }, complete]);
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

describe("contributor lists", () => {
  it("are complete when they hold their total, and capped otherwise", () => {
    const entry = { node: "n_a", score: 1 };
    expect(isComplete({ entries: [entry], total: 1 })).toBe(true);
    expect(isComplete({ entries: [entry], total: 60 })).toBe(false);
  });
});
