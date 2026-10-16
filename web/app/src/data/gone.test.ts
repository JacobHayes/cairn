// A18, A19: a refusal that says the edit's subject no longer exists is told from one that is
// merely refused, on the shapes the engine itself answers (the page's engine, seeded with the
// fixtures, as the authoring tests use it).
import type { Patch, Rejection, Schema } from "@cairn/client";
import type { InBrowserHost } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { planRemoval, removalMutations } from "../authoring/cascade.ts";
import { applyToJourney, journeyAfter, seeded } from "../authoring/engine.test-support.ts";
import { treeOf } from "../authoring/graph.ts";
import { goneOf } from "./write-machine.ts";

const JOURNEY = "j_vendor_eval";

type Mutation = Schema<"Mutation">;

describe("goneOf", () => {
  let host: InBrowserHost;
  let removed: string;
  beforeAll(async () => {
    host = await seeded();
    const text = host.documentText(JOURNEY);
    const plan = planRemoval(treeOf((JSON.parse(text) as Schema<"DomainDocument">).journey.graph), "n_partner_led");
    removed = journeyAfter(host, JOURNEY, plan === undefined ? [] : removalMutations(plan));
  });

  /** The patch for `mutations` and the rejection the engine answers it with, on the document `removed`. */
  function refused(mutations: Mutation[]): { patch: Patch; rejection: Rejection } {
    const outcome = applyToJourney(host, JOURNEY, mutations, removed);
    if (outcome.accepted) {
      throw new Error("the engine accepted what the test expects it to refuse");
    }
    const base_revision = (JSON.parse(removed) as Schema<"DomainDocument">).journey.revision;
    return { patch: { id: "p_test", target: { journey: JOURNEY }, base_revision, mutations }, rejection: { rejection: "invalid", violations: outcome.violations } };
  }

  it("says a node removed since the author opened it is gone", () => {
    const { patch, rejection } = refused([{ op: "set_node_field", node: "n_partner_led", value: { title: "Renamed" } }]);
    expect(goneOf(patch, rejection)).toEqual({ node: "n_partner_led" });
  });

  it("does not say a live node is gone because it points to a removed one", () => {
    const { patch, rejection } = refused([{ op: "add_edge", edge: { node: "n_plan", requires: "n_partner_led" } }]);
    expect(goneOf(patch, rejection)).toBeUndefined();
  });

  it("does not say a node is gone because some other rule refused the edit", () => {
    const { patch, rejection } = refused([{ op: "add_edge", edge: { node: "n_plan", requires: "n_plan" } }]);
    expect(goneOf(patch, rejection)).toBeUndefined();
  });

  const patchFor = (target: Patch["target"]): Patch => ({ id: "p_test", target, base_revision: 3, mutations: [] });
  const staleAt = (domain: Schema<"Domain">, current: number): Rejection => ({ rejection: "stale", conflicts: [{ of: { domain }, expected: 3, current }], intervening: [] });

  it("says a journey or route answered at revision 0 is gone, and one that moved on is not", () => {
    expect(goneOf(patchFor({ journey: "j_one" }), staleAt({ journey: "j_one" }, 0))).toEqual({ journey: "j_one" });
    expect(goneOf(patchFor({ route: "r_one" }), staleAt({ route: "r_one" }, 0))).toEqual({ route: "r_one" });
    expect(goneOf(patchFor({ journey: "j_one" }), staleAt({ journey: "j_one" }, 7))).toBeUndefined();
  });

  it("does not take another domain's revision 0 for the write's own", () => {
    expect(goneOf(patchFor({ journey: "j_one" }), staleAt({ journey: "j_other" }, 0))).toBeUndefined();
    expect(goneOf(patchFor("deployment"), staleAt("deployment", 0))).toBeUndefined();
  });
});
