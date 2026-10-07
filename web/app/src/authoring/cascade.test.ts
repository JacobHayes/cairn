// A18 on the fixtures, against the engine: a removal names its subtree, its incident edges,
// and what hangs on them, and carries the rewrite of every reference that would dangle (a
// condition, a snooze, a message draft's placeholder); the engine accepts it with its cascade
// and refuses it without. Removing a role rewrites what names it first.
import type { Schema } from "@cairn/client";
import { beforeAll, describe, expect, it } from "vitest";
import type { InBrowserHost } from "@cairn/wasm";

import { planKindRemoval, planRemoval, planRoleRemoval, removalMutations, withoutDecisions, withoutPlaceholders } from "./cascade.ts";
import { applyToDraft, applyToJourney, journeyAfter, seeded } from "./engine.test-support.ts";
import { treeOf, type Mutation } from "./graph.ts";

const JOURNEY = "j_vendor_eval";

describe("planRemoval (A18)", () => {
  let host: InBrowserHost;
  beforeAll(async () => {
    host = await seeded();
  });
  // The review opening waits on a workload's piece, so removing the workload leaves a snooze dangling.
  let text: string;
  beforeAll(() => {
    text = journeyAfter(host, JOURNEY, [{ op: "snooze", node: "n_review_opens", until: { node: "n_workload_query" } }]);
  });
  const plan = (key: string) => {
    const found = planRemoval(treeOf((JSON.parse(text) as Schema<"DomainDocument">).journey.graph), key);
    if (found === undefined) {
      throw new Error(`no ${key}`);
    }
    return found;
  };

  const cases: [string, string, string][] = [
    ["a decision a condition names", "n_comparison_set", "condition"],
    ["a placeholder a snooze waits on beneath", "n_workload", "snooze"],
    ["a decision a message draft names", "n_purpose", "message_draft"],
  ];

  it.each(cases)("removing %s is accepted with its cascade", (_, key, what) => {
    const removal = plan(key);
    expect(removal.dangling.map((each) => each.what)).toContain(what);
    expect(applyToJourney(host, JOURNEY, removalMutations(removal), text)).toEqual({ accepted: true });
  });

  it.each(cases)("removing %s is refused without its cascade", (_, key) => {
    const removal = plan(key);
    const bare: Mutation[] = [{ op: "remove_node", removal: removal.removal }];
    const outcome = applyToJourney(host, JOURNEY, bare, text);
    expect(outcome.accepted).toBe(false);
  });

  it("names the subtree, every incident edge, and what hangs on them", () => {
    const setup = plan("n_setup").removal;
    expect(setup.descendants).toEqual(expect.arrayContaining(["n_access", "n_plan", "n_plan_draft", "n_workload_ingest"]));
    expect(setup.edges).toEqual(expect.arrayContaining([{ node: "n_comparison_set", requires: "n_plan" }, { node: "n_plan", requires: "n_access" }]));
    expect(setup.resources).toEqual(expect.arrayContaining(["a_access_request", "a_plan_example_one"]));
    expect(setup.annotations).toEqual(["a_access_note"]);
    expect(applyToJourney(host, JOURNEY, removalMutations(plan("n_setup")), text)).toEqual({ accepted: true });
  });

  it("a removal that misses what was added since is refused rather than widened", () => {
    const removal = plan("n_workload").removal;
    const narrowed = { ...removal, descendants: (removal.descendants ?? []).slice(1) };
    const outcome = applyToJourney(host, JOURNEY, [{ op: "unsnooze", node: "n_review_opens" }, { op: "remove_node", removal: narrowed }], text);
    expect(!outcome.accepted && outcome.codes).toEqual(["removal_widened"]);
  });
});

describe("removing a role or a kind (A18)", () => {
  let host: InBrowserHost;
  beforeAll(async () => {
    host = await seeded();
  });

  it("rewrites every participation, filling decision, default owner, and draft placeholder naming the role", () => {
    const graph = host.routeVersion("vendor-evaluation", 1).graph;
    const tree = treeOf(graph);
    const dangling = planRoleRemoval(tree, "r_eval_owner");
    expect(new Set(dangling.map((each) => each.what))).toEqual(new Set(["fills_role", "default_owner", "message_draft"]));
    const remove: Mutation = { op: "remove_role", role: "r_eval_owner" };
    expect(applyToDraft(host.engine, host.deployment(), graph, [...dangling.map((each) => each.mutation), remove])).toEqual({ accepted: true });
    expect(applyToDraft(host.engine, host.deployment(), graph, [remove]).accepted).toBe(false);
  });

  it("clears every participation of a kind before removing it", () => {
    const graph = host.routeVersion("vendor-evaluation", 1).graph;
    const dangling = planKindRemoval(treeOf(graph), "k_reviewer");
    expect(dangling.map((each) => each.node)).toEqual(["n_final_report"]);
    const remove: Mutation = { op: "remove_participation_kind", kind: "k_reviewer" };
    expect(applyToDraft(host.engine, host.deployment(), graph, [...dangling.map((each) => each.mutation), remove])).toEqual({ accepted: true });
  });
});

describe("rewrites", () => {
  it("drops the clauses naming a removed decision, and the combinators they leave empty", () => {
    const kept = { answered: "n_kept" };
    expect(withoutDecisions({ all: [kept, { not: { answered: "n_gone" } }] }, new Set(["n_gone"]))).toEqual({ all: [kept] });
    expect(withoutDecisions({ any: [{ answered: "n_gone" }] }, new Set(["n_gone"]))).toBeUndefined();
  });

  it("drops a draft's placeholders naming what is removed, by key", () => {
    const draft = "Hi {{roles.r_lead.name}}: {{answers.n_gone}} and {{answers.n_kept}} for {{journey.name}}.";
    expect(withoutPlaceholders(draft, new Set(["n_gone", "r_lead"]))).toBe("Hi :  and {{answers.n_kept}} for {{journey.name}}.");
  });
});
