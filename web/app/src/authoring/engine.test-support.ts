// The real engine for authoring's unit tests: the browser host's module (web/wasm/dist, which
// rung 6 builds before its web unit tests) loaded in Node, seeded with every fixture as the
// in-browser host is. The editors' rules (which fields a kind has, which operators a decision
// takes, what a removal must rewrite) are checked against what the engine accepts, so they
// cannot drift from it unnoticed.
import { readFileSync } from "node:fs";

import type { Schema } from "@cairn/client";
import { HostFailure, InBrowserHost, type Engine } from "@cairn/wasm";

import type { Graph, Mutation } from "./graph.ts";

let started: Promise<InBrowserHost> | undefined;

/** The seeded in-browser host, started once per test file. */
export function seeded(): Promise<InBrowserHost> {
  started ??= InBrowserHost.start(readFileSync(new URL("../../../wasm/dist/bindgen/cairn_wasm_bg.wasm", import.meta.url)), () => "2026-10-06T12:00:00Z");
  return started;
}

const actor = { user: "u_local" };
const at = "2026-10-06T12:00:00Z";

/** What the engine answered a local apply: accepted, or the codes of its violations. */
export type Outcome = { accepted: true } | { accepted: false; codes: Schema<"ViolationCode">[]; violations: Schema<"Violation">[] };

function outcomeOf(call: () => unknown): Outcome {
  try {
    call();
    return { accepted: true };
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "rejected" && thrown.reason.rejection.rejection === "invalid") {
      const violations = thrown.reason.rejection.violations;
      return { accepted: false, codes: violations.map((violation) => violation.code), violations };
    }
    throw thrown;
  }
}

/** A route whose draft holds `graph`, at revision 1, as a route read answers it. */
export function routeHolding(graph: Graph, id = "authored"): Schema<"Route"> {
  return { header: { id, name: "Authored" }, revision: 1, draft: { graph } };
}

/** `mutations` applied locally to a route whose draft is `graph`. */
export function applyToDraft(engine: Engine, deployment: Schema<"Deployment">, graph: Graph, mutations: Mutation[]): Outcome {
  const route = routeHolding(graph);
  const patch = { id: "p_test", target: { route: route.header.id }, base_revision: route.revision, mutations };
  return outcomeOf(() => engine.applyRoute({ route, deployment, patch, today: "2026-10-06", at, actor }));
}

/** `mutations` applied locally to a journey's document text (a fixture's as the root holds it, by default). */
export function applyToJourney(host: InBrowserHost, journey: string, mutations: Mutation[], text = host.documentText(journey)): Outcome {
  const patch = { id: "p_test", target: { journey }, base_revision: (JSON.parse(text) as Schema<"DomainDocument">).journey.revision, mutations };
  return outcomeOf(() => host.engine.apply(text, { patch, at, actor }));
}

/** The document text `mutations` leave a journey's document text in; they must be accepted. */
export function journeyAfter(host: InBrowserHost, journey: string, mutations: Mutation[], text = host.documentText(journey)): string {
  const patch = { id: "p_test", target: { journey }, base_revision: (JSON.parse(text) as Schema<"DomainDocument">).journey.revision, mutations };
  return JSON.stringify(host.engine.apply(text, { patch, at, actor }).document);
}

/** Fixture journey `journey`'s document as the root holds it. */
export function journeyDocument(host: InBrowserHost, journey: string): Schema<"DomainDocument"> {
  return JSON.parse(host.documentText(journey)) as Schema<"DomainDocument">;
}
