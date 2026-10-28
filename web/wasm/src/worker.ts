// The derive worker (ARCHITECTURE, Web UI: data flow): its own instance of the module,
// deriving each domain document the page posts and answering projections, previews, and
// local applies over the held derivation. Messages are in ./protocol.ts. Once the module
// traps, the worker drops what it holds and refuses every later request with that reason (a
// panic aborts).
//
// A derivation is owned by the load that made it: each load is numbered and answers its
// handle, and a release frees a derivation only while the load it names is the one held. The
// worker answers messages in order and runs a load to completion before the next message, so
// an owner replaced by a successor can only have posted its load before the successor's, and
// its release, whenever it arrives, names a load no longer held.
import { loadEngine, type DerivedDocument, type DerivedRoute, type Engine } from "./engine.ts";
import type { Envelope, Reply, WorkerRequest } from "./protocol.ts";
import { HostFailure, failureOf, stoppedBy, type DerivationHandle, type DerivationKey, type Loaded, type PreviewRequest, type RouteDeriveRequest } from "./types.ts";

/** The parts of a dedicated worker's global scope this script uses. */
interface WorkerScope {
  onmessage: ((event: MessageEvent<Envelope>) => void) | null;
  postMessage(message: Reply): void;
}

/** What both kinds of derivation answer and release. */
interface Derivation {
  projectText: DerivedDocument["projectText"];
  free(): void;
}

/** A journey's document as posted, with its derivation. */
interface HeldJourney {
  kind: "journey";
  document: string;
  derivation: DerivedDocument;
  /** The number of the load that made the derivation. */
  load: number;
}

/** A route's graph derived as a journey in which nothing has happened, under a slot of its own. */
interface HeldRoute {
  kind: "route";
  derivation: DerivedRoute;
  load: number;
}

type Held = HeldJourney | HeldRoute;

const scope = globalThis as unknown as WorkerScope;
const held = new Map<string, Held>();
let engine: Engine | undefined;
let loads = 0;

/** The day in a route's key: a route has no calendar date, and nothing it answers depends on one. */
const ROUTE_DAY = "2000-01-01";

function holdingAny(name: string): Held {
  const found = held.get(name);
  if (found === undefined) {
    throw new HostFailure({ error: "missing", message: `no document for ${name} is loaded` });
  }
  return found;
}

/** The journey held under `name`; a route's slot is refused as missing, since a journey's reads do not apply to it. */
function holding(name: string): HeldJourney {
  const found = holdingAny(name);
  if (found.kind !== "journey") {
    throw new HostFailure({ error: "missing", message: `no document for ${name} is loaded` });
  }
  return found;
}

/** The route held under `name`. */
function holdingRoute(name: string): HeldRoute {
  const found = holdingAny(name);
  if (found.kind !== "route") {
    throw new HostFailure({ error: "missing", message: `no route graph for ${name} is loaded` });
  }
  return found;
}

/** The derivation held under `name`, whichever kind. */
function derivationOf(name: string): Derivation {
  return holdingAny(name).derivation;
}

function ready(): Engine {
  if (engine === undefined) {
    throw new HostFailure({ error: "missing", message: "the worker was not initialized" });
  }
  return engine;
}

/** Whether `posted` is older than `held`: an earlier revision, deployment, or day. */
function older(posted: DerivationKey, held: DerivationKey): boolean {
  const order = (key: DerivationKey): [number, number, string] => [key.revision, key.deployment_revision, key.today];
  const [a, b] = [order(posted), order(held)];
  return a[0] !== b[0] ? a[0] < b[0] : a[1] !== b[1] ? a[1] < b[1] : a[2] < b[2];
}

/**
 * Derives a document and holds it in place of its journey's previous one, under a new load
 * number; one older than what is held is refused, so a late reply to an earlier fetch never
 * replaces a newer view.
 */
function load(document: string, slot?: string): string {
  const derivation = ready().derive(document);
  const name = slot ?? derivation.key.journey;
  const current = held.get(name);
  // A slot is replaced by its next load whatever its revision: it holds a preview, not a revision.
  if (slot === undefined && current?.kind === "journey" && older(derivation.key, current.derivation.key)) {
    derivation.free();
    throw new HostFailure({ error: "superseded", held: current.derivation.key, posted: derivation.key });
  }
  current?.derivation.free();
  loads += 1;
  held.set(name, { kind: "journey", document, derivation, load: loads });
  const handle: DerivationHandle = { journey: name, load: loads };
  // A slot's key names the slot and its load, so what is read from one preview is never read from the next.
  const key: DerivationKey = slot === undefined ? derivation.key : { ...derivation.key, journey: slot, generation: loads };
  return JSON.stringify({ key, handle });
}

/**
 * Derives a route's graph and holds it under `slot` in place of what the slot held. A slot is
 * the page's to name freshly for each load (`route:<id>:<draft|n>#<n>`), so a read through an
 * earlier load's key is never answered from the load that replaced it.
 */
function loadRoute(request: RouteDeriveRequest, slot: string): string {
  const derivation = ready().deriveRoute(request);
  held.get(slot)?.derivation.free();
  loads += 1;
  held.set(slot, { kind: "route", derivation, load: loads });
  const handle: DerivationHandle = { journey: slot, load: loads };
  const key: DerivationKey = { journey: slot, revision: 0, deployment_revision: request.deployment.revision, today: ROUTE_DAY, generation: loads };
  return JSON.stringify({ key, handle });
}

/**
 * A proposal previewed against the journey held under `journey`, which must still be the
 * derivation `posted` was read from; the journey it would leave is derived and held under `slot`.
 */
function preview(journey: string, posted: DerivationKey, request: PreviewRequest, slot: string): string {
  const source = holding(journey);
  const now = source.derivation.key;
  if (now.revision !== posted.revision || now.deployment_revision !== posted.deployment_revision || now.today !== posted.today) {
    throw new HostFailure({ error: "superseded", held: now, posted });
  }
  const answer = ready().preview(source.document, request);
  if (answer.after?.destination !== "journey") {
    // Nothing to derive; what the slot held stays until its owner releases it.
    return JSON.stringify(answer);
  }
  const loaded = JSON.parse(load(JSON.stringify(answer.after.document), slot)) as Loaded;
  return JSON.stringify({ ...answer, loaded });
}


/** Frees the derivation `handle` owns, if its load is still the one held. */
function release(handle: DerivationHandle): void {
  const current = held.get(handle.journey);
  if (current?.load === handle.load) {
    current.derivation.free();
    held.delete(handle.journey);
  }
}

/** Drops every held derivation once the module stopped. */
function dropAll(): void {
  for (const { derivation } of held.values()) {
    derivation.free();
  }
  held.clear();
}

async function answer(request: WorkerRequest): Promise<string> {
  switch (request.op) {
    case "init":
      engine = await loadEngine(request.wasm);
      return "";
    case "load":
      return load(request.document, request.slot);
    case "derived":
      return holding(request.journey).derivation.derivedText();
    case "load_route":
      return loadRoute(request.request, request.slot);
    case "project":
      return derivationOf(request.journey).projectText(request.request);
    case "render_draft":
      return holding(request.journey).derivation.renderDraftText(request.request);
    case "preview":
      return preview(request.journey, request.key, request.request, request.slot);
    case "apply":
      return ready().applyText(holding(request.journey).document, request.request);
    case "release":
      release(request.handle);
      return "";
    case "route_notices":
      return ready().routeNoticesText(request.request);
    case "route_schedule":
      return holdingRoute(request.journey).derivation.scheduleText();
    case "apply_route":
      return ready().applyRouteText(request.request);
    case "read_route_file":
      return ready().readRouteFileText(request.text);
    case "route_file_text":
      return ready().routeFileText(request.file);
    case "condition_problem":
      return ready().conditionProblemText(request.clause);
    case "node_form_problems":
      return ready().nodeFormProblemsText(request.values);
    case "restore_nodes":
      return ready().restoreNodesText(request.journey, request.version, request.key, request.seed);
    case "removal_plan":
      return ready().removalPlanText(request.graph, request.target);
    case "draft_written":
      return ready().draftWrittenText(request.graph, request.stored);
    case "draft_stored":
      return ready().draftStoredText(request.graph, request.written);
    case "memory":
      return String(ready().memoryBytes());
  }
}

scope.onmessage = (event) => {
  const { id } = event.data;
  const reply = async (): Promise<Reply> => {
    const before = stoppedBy();
    if (before !== undefined) {
      return { id, ok: false, error: before };
    }
    try {
      return { id, ok: true, text: await answer(event.data) };
    } catch (thrown) {
      const { reason } = failureOf(thrown);
      if (stoppedBy() !== undefined) {
        dropAll();
      }
      return { id, ok: false, error: reason };
    }
  };
  void reply().then((message) => {
    scope.postMessage(message);
  });
};
