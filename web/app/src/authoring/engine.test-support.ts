// The real engine for authoring's unit tests: the browser's module (web/wasm/dist, which the
// web-unit step builds before its tests) loaded in Node, with every fixture as a running
// `cairn demo` holds it. The editors' rules (which fields a kind has, which operators a decision
// takes, what a removal must rewrite) are checked against what the engine accepts, so they
// cannot drift from it unnoticed.
import { readFileSync } from "node:fs";

import type { Schema } from "@cairn/client";
import { HostFailure, compileEngine, loadEngine, type Engine } from "@cairn/wasm";

import { startNamedDemo } from "../demo-server.test-support.ts";

import type { Graph, Mutation } from "./graph.ts";

/** The day the fixtures are read on: the server's today is replaced with it, so what depends on the date does not move. */
const TODAY = "2026-10-06";

/** A fetch of the demo's JSON; fails when it does not answer. */
async function fetched<T>(origin: string, path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${origin}${path}`, init);
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${path}: ${String(response.status)} ${text}`);
  }
  return JSON.parse(text) as T;
}

/** A journey's document as the demo answers it, its today fixed. */
async function documentOf(origin: string, journey: string): Promise<string> {
  const document = await fetched<Schema<"DomainDocument">>(origin, `/api/journeys/${journey}/document`);
  return JSON.stringify({ ...document, inputs: { ...document.inputs, today: TODAY } });
}

/** The engine, and the fixtures as a `cairn demo` of the file's own holds them. */
export class Fixtures {
  readonly engine: Engine;
  readonly #origin: string;
  readonly #documents: Map<string, string>;
  readonly #versions: Map<string, Schema<"RouteVersion">>;
  readonly #deployment: Schema<"Deployment">;

  private constructor(engine: Engine, origin: string, documents: Map<string, string>, versions: Map<string, Schema<"RouteVersion">>, deployment: Schema<"Deployment">) {
    this.engine = engine;
    this.#origin = origin;
    this.#documents = documents;
    this.#versions = versions;
    this.#deployment = deployment;
  }

  /** Starts a demo, loads the engine, and reads every journey, route version, and the deployment. */
  static async load(): Promise<Fixtures> {
    const { origin } = await startNamedDemo();
    const engine = await wasmEngine();
    const documents = new Map<string, string>();
    for (const journey of (await fetched<Schema<"JourneyPage">>(origin, "/api/journeys")).items) {
      documents.set(journey.id, await documentOf(origin, journey.id));
    }
    const versions = new Map<string, Schema<"RouteVersion">>();
    for (const route of (await fetched<Schema<"RoutePage">>(origin, "/api/routes")).items) {
      for (let version = 1; version <= (route.latest_version ?? 0); version += 1) {
        versions.set(`${route.header.id}@${String(version)}`, await fetched(origin, `/api/routes/${route.header.id}/versions/${String(version)}`));
      }
    }
    return new Fixtures(engine, origin, documents, versions, await fetched(origin, "/api/deployment"));
  }

  /** The ids of the journeys the demo was seeded with, and those patched in since. */
  journeyIds(): string[] {
    return [...this.#documents.keys()];
  }

  /** A journey's document text. */
  documentText(journey: string): string {
    const text = this.#documents.get(journey);
    if (text === undefined) {
      throw new Error(`no journey ${journey}`);
    }
    return text;
  }

  /** The deployment. */
  deployment(): Schema<"Deployment"> {
    return this.#deployment;
  }

  /** A published version of a route. */
  routeVersion(route: string, version: number): Schema<"RouteVersion"> {
    const found = this.#versions.get(`${route}@${String(version)}`);
    if (found === undefined) {
      throw new Error(`no version ${String(version)} of ${route}`);
    }
    return found;
  }

  /** Sends a patch to the demo, which must apply it, and holds the journey's document it leaves. */
  async patch(request: Schema<"PatchRequest">): Promise<void> {
    const target = request.patch.target;
    if (typeof target === "string" || !("journey" in target)) {
      throw new Error("only a journey's patch is sent");
    }
    await fetched(this.#origin, `/api/journeys/${target.journey}/patches`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(request) });
    this.#documents.set(target.journey, await documentOf(this.#origin, target.journey));
  }
}

let loaded: Promise<Engine> | undefined;

/** The engine alone, loaded once per test file, for the tests that need no journey. */
export function wasmEngine(): Promise<Engine> {
  loaded ??= compileEngine(readFileSync(new URL("../../../wasm/dist/bindgen/cairn_wasm_bg.wasm", import.meta.url))).then(loadEngine);
  return loaded;
}

let started: Promise<Fixtures> | undefined;

/** The engine and the fixtures, loaded once per test file. */
export function seeded(): Promise<Fixtures> {
  started ??= Fixtures.load();
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

/** The engine's plan for removing `target` from `graph`, or fails when a node asked for is not there. */
export function removalPlan(host: Fixtures, graph: Graph, target: Schema<"RemovalTarget">): Schema<"RemovalPlan"> {
  const plan = JSON.parse(host.engine.removalPlanText(graph, target)) as Schema<"RemovalPlan"> | null;
  if (plan === null) {
    throw new Error(`no ${JSON.stringify(target)} to remove`);
  }
  return plan;
}

/** What the plan sends, in order: the rewrites first, then the removal of a node. */
export function planMutations(plan: Schema<"RemovalPlan">): Mutation[] {
  return [...(plan.dangling ?? []).map((each) => each.mutation), ...(plan.removal == null ? [] : [{ op: "remove_node" as const, removal: plan.removal }])];
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

/** `mutations` applied locally to a journey's document text (a fixture's, by default). */
export function applyToJourney(host: Fixtures, journey: string, mutations: Mutation[], text = host.documentText(journey)): Outcome {
  const patch = { id: "p_test", target: { journey }, base_revision: (JSON.parse(text) as Schema<"DomainDocument">).journey.revision, mutations };
  return outcomeOf(() => host.engine.apply(text, { patch, at, actor }));
}

/** The document text `mutations` leave a journey's document text in; they must be accepted. */
export function journeyAfter(host: Fixtures, journey: string, mutations: Mutation[], text = host.documentText(journey)): string {
  const patch = { id: "p_test", target: { journey }, base_revision: (JSON.parse(text) as Schema<"DomainDocument">).journey.revision, mutations };
  return JSON.stringify(host.engine.apply(text, { patch, at, actor }).document);
}

/** A journey as the tab holds it once derived: its view, projections and a way to release the derivation. */
export interface DerivedJourney {
  ready: Extract<import("../data/journeys.ts").JourneyView, { status: "ready" }>;
  project: <R extends import("@cairn/wasm").ProjectionRequest>(request: R) => import("@cairn/wasm").ProjectionAnswer<R>;
  free: () => void;
}

/** The journey in document `text` (a fixture's, or one `journeyAfter` returned), derived by the real engine. */
export function derivedJourney(host: Fixtures, text: string): DerivedJourney {
  const document = JSON.parse(text) as Schema<"DomainDocument">;
  const derivation = host.engine.derive(text);
  return {
    ready: { status: "ready", journey: document.journey, inputs: document.inputs, key: derivation.key, derived: derivation.derived() },
    project: (request) => derivation.project(request),
    free: () => { derivation.free(); },
  };
}

/** A journey with nothing in it, as the demo makes one: the base of the journeys `builtJourney` builds. */
let blank: string | undefined;

/**
 * A new journey named `name` with `id`, patched with `mutations` and derived by the real
 * engine, for the unit tests of the screens that read a derived journey. Synchronous so a
 * test support can return it: `then` is a second patch on the result (what a patch cannot do in
 * one step, such as ruling out work it finished); the engine and an empty journey are loaded
 * once, with top-level await, by the support.
 */
export function builtJourney(id: string, name: string, mutations: Mutation[], then: Mutation[] = []): import("../data/journeys.ts").JourneyView & { status: "ready" } {
  if (engineHost === undefined || blank === undefined) throw new Error("the engine is not loaded: await loadedEngine() before building a journey");
  const empty = JSON.parse(blank) as Schema<"DomainDocument">;
  const created = JSON.stringify({ ...empty, journey: { ...empty.journey, header: { ...empty.journey.header, id, name, created_at: `${TODAY}T12:00:00Z`, created_on: TODAY } } });
  const first = journeyAfter(engineHost, id, mutations, created);
  const derived = derivedJourney(engineHost, then.length === 0 ? first : journeyAfter(engineHost, id, then, first));
  const { ready } = derived;
  derived.free();
  return ready;
}

let engineHost: Fixtures | undefined;

/** Loads the engine and an empty journey for `builtJourney`; a test support awaits it at its top level. */
export async function loadedEngine(): Promise<void> {
  engineHost = await seeded();
  if (blank === undefined) {
    await engineHost.patch({ patch: { id: "p_create_blank", target: { journey: "j_blank" }, base_revision: 0, mutations: [{ op: "create_journey", name: "Blank" }] } });
    blank = engineHost.documentText("j_blank");
  }
}
