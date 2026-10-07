// B7 against the engine: the resolutions the review offers each kind of conflict are exactly
// the ones the engine accepts. Every resolution is tried on one conflict of each kind in a
// proposal previewed by the real engine over the vendor evaluation; the engine marks a choice
// it does not offer as `not_offered`, and the review must agree on every pair.
import type { Schema } from "@cairn/client";
import type { InBrowserHost } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { journeyDocument, seeded } from "../authoring/engine.test-support.ts";
import { offers, type Conflict, type Resolution } from "./model.ts";

const JOURNEY = "j_vendor_eval";
const at = "2026-10-06T12:00:00Z";

const role = (multi: boolean): Schema<"RoleResolved"> => ({ key: "r_findings_reviewer", id: "findings_reviewer", title: "Findings reviewer", multi });
const kind: Schema<"ParticipationKindResolved"> = { key: "k_reviewer", id: "reviewer", title: "Reviewer" };

/** One conflict of each kind and each variant whose offers differ, over the journey's own keys. */
function conflicts(access: Schema<"Node">): [string, Conflict][] {
  return [
    ["an edited field", { about: "field", node: "n_access", journey: { title: "Ours" }, route: { title: "Theirs" }, dangling: false }],
    ["a field whose route value dangles", { about: "field", node: "n_access", journey: { title: "Ours" }, route: { title: "Theirs" }, dangling: true }],
    ["an edge", { about: "edge", edge: { node: "n_findings", requires: "n_kickoff" }, journey: true, route: false }],
    ["a participation", { about: "participation", node: "n_final_report", kind: "k_reviewer", journey: ["e_lead"], route: "r_findings_reviewer" }],
    ["a resource", { about: "resource", node: "n_access", resource: "a_absent", dangling: false }],
    ["a resource whose route value dangles", { about: "resource", node: "n_access", resource: "a_absent", dangling: true }],
    ["a shape", { about: "shape", journey: access, route: { ...access, kind: "action" }, answered: false, dangling: false }],
    ["a shape whose route value dangles", { about: "shape", journey: access, route: { ...access, kind: "action" }, answered: false, dangling: true }],
    ["an answer naming a removed choice", { about: "answer", decision: "n_purpose", answer: { single_choice: "purchase" }, choices: ["buy", "research-only"] }],
    ["a role the route removed", { about: "role", role: "r_findings_reviewer", journey: role(false), route: null, references: [] }],
    ["a role the route changed", { about: "role", role: "r_findings_reviewer", journey: role(true), route: role(false), references: [] }],
    ["a role too narrow for its fill", { about: "role", role: "r_findings_reviewer", journey: role(true), route: role(false), references: [{ fill: { entities: ["e_lead", "e_reviewer"] } }] }],
    ["a kind the route removed", { about: "kind", kind: "k_reviewer", journey: kind, route: null, references: {} }],
    ["a kind the route changed", { about: "kind", kind: "k_reviewer", journey: kind, route: { ...kind, multi: true }, references: {} }],
    ["the default owner", { about: "default_owner", journey: "r_eval_owner", route: "r_stakeholders" }],
  ];
}

const resolutions: Resolution[] = [
  "keep_journey",
  "take_route",
  "clear_state",
  "reopen",
  "remove",
  { remap_role: { role: "r_eval_owner" } },
  { remap_kind: { kind: "k_informed" } },
  { map_choices: { map: { purchase: "buy" } } },
  { map_choices: { map: { purchase: "gone" } } },
];

/** Whether the engine refuses `resolution` on `conflict` as not offered, previewing a proposal holding just that item. */
function engineRefuses(host: InBrowserHost, text: string, revision: number, conflict: Conflict, resolution: Resolution): boolean {
  const proposal: Schema<"Proposal"> = {
    id: "pr_offers",
    destination: { journey: JOURNEY },
    revision: 1,
    status: "open",
    created_by: "u_local",
    created_at: at,
    draft: { title: "Offers", destination_revision: revision, items: [{ item: "conflict", conflict, resolution }] },
  };
  const preview = host.engine.preview(text, { proposal, at, actor: { user: "u_local" } });
  return (preview.unresolved ?? []).some((item) => item.item === 0 && item.reason === "not_offered");
}

describe("offers (B7)", () => {
  let host: InBrowserHost;
  beforeAll(async () => {
    host = await seeded();
  });

  it("offers exactly what the engine accepts, for every kind of conflict and resolution", () => {
    const text = host.documentText(JOURNEY);
    const document = journeyDocument(host, JOURNEY);
    const access = (document.journey.graph.nodes ?? []).find((node) => node.key === "n_access");
    if (access === undefined) {
      throw new Error("the fixture holds the access deliverable");
    }
    const disagreements = conflicts(access).flatMap(([name, conflict]) =>
      resolutions.flatMap((resolution) => {
        const ours = offers(conflict, resolution);
        const engines = !engineRefuses(host, text, document.journey.revision, conflict, resolution);
        return ours === engines ? [] : [`${name}, ${JSON.stringify(resolution)}: the review says ${String(ours)}, the engine ${String(engines)}`];
      }),
    );
    expect(disagreements).toEqual([]);
  });
});
