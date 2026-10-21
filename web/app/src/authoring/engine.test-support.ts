// The real engine for authoring's unit tests: the browser host's module (web/wasm/dist, which
// the web-unit step builds before its tests) loaded in Node, seeded with every fixture as the
// in-browser host is. The editors' rules (which fields a kind has, which operators a decision
// takes, what a removal must rewrite) are checked against what the engine accepts, so they
// cannot drift from it unnoticed.
import { readFileSync } from "node:fs";

import type { Schema } from "@cairn/client";
import { HostFailure, InBrowserHost, compileEngine, type Engine } from "@cairn/wasm";

import type { Graph, Mutation } from "./graph.ts";

let started: Promise<InBrowserHost> | undefined;

/** The seeded in-browser host, started once per test file. */
export function seeded(): Promise<InBrowserHost> {
  started ??= compileEngine(readFileSync(new URL("../../../wasm/dist/bindgen/cairn_wasm_bg.wasm", import.meta.url))).then((module) => InBrowserHost.start(module, () => "2026-10-06T12:00:00Z"));
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

/** A journey as the tab holds it once derived: its view, projections and a way to release the derivation. */
export interface DerivedJourney {
  ready: Extract<import("../data/journeys.ts").JourneyView, { status: "ready" }>;
  project: <R extends import("@cairn/wasm").ProjectionRequest>(request: R) => import("@cairn/wasm").ProjectionAnswer<R>;
  free: () => void;
}

/** The journey in document `text` (a fixture's, or one `journeyAfter` returned), derived by the real engine. */
export function derivedJourney(host: InBrowserHost, text: string): DerivedJourney {
  const document = JSON.parse(text) as Schema<"DomainDocument">;
  const derivation = host.engine.derive(text);
  return {
    ready: { status: "ready", journey: document.journey, inputs: document.inputs, key: derivation.key, derived: derivation.derived() },
    project: (request) => derivation.project(request),
    free: () => { derivation.free(); },
  };
}

let built: Promise<InBrowserHost> | undefined;

/**
 * A new journey named `name` with `id`, patched with `mutations` and derived by the real
 * engine, for the unit tests of the screens that read a derived journey. Synchronous so a
 * test support can return it: the engine is loaded once, with top-level await, by the support.
 */
export function builtJourney(id: string, name: string, mutations: Mutation[]): import("../data/journeys.ts").JourneyView & { status: "ready" } {
  if (engineHost === undefined) throw new Error("the engine is not loaded: await loadedEngine() before building a journey");
  engineHost.patch({ patch: { id: `p_create_${id}`, target: { journey: id }, base_revision: 0, mutations: [{ op: "create_journey", name }] } });
  const derived = derivedJourney(engineHost, journeyAfter(engineHost, id, mutations));
  const { ready } = derived;
  derived.free();
  return ready;
}

let engineHost: InBrowserHost | undefined;

/** Loads the engine for `builtJourney`; a test support awaits it at its top level. */
export async function loadedEngine(): Promise<void> {
  built ??= seeded();
  engineHost = await built;
}
