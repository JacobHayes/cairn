// The derive worker (ARCHITECTURE, Web UI: data flow): its own instance of the module,
// deriving each domain document the page posts and answering projections, previews, and
// local applies over the held derivation. Messages are in ./protocol.ts. Once a document
// from another engine version is posted, or the module traps, the worker drops what it holds
// and refuses every later request with that reason (version skew; a panic aborts).
import { loadEngine, type DerivedDocument, type Engine } from "./engine.ts";
import type { Envelope, Reply, WorkerRequest } from "./protocol.ts";
import { HostFailure, failureOf, stoppedBy, type DerivationKey } from "./types.ts";

/** The parts of a dedicated worker's global scope this script uses. */
interface WorkerScope {
  onmessage: ((event: MessageEvent<Envelope>) => void) | null;
  postMessage(message: Reply): void;
}

/** A journey's document as posted, with its derivation. */
interface Held {
  document: string;
  derivation: DerivedDocument;
}

const scope = globalThis as unknown as WorkerScope;
const held = new Map<string, Held>();
let engine: Engine | undefined;

function holding(journey: string): Held {
  const found = held.get(journey);
  if (found === undefined) {
    throw new HostFailure({ error: "missing", message: `no document for ${journey} is loaded` });
  }
  return found;
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
 * Derives a document and holds it in place of its journey's previous one; one older than
 * what is held is refused, so a late reply to an earlier fetch never replaces a newer view.
 */
function load(document: string): string {
  const derivation = ready().derive(document);
  const journey = derivation.key.journey;
  const current = held.get(journey);
  if (current !== undefined && older(derivation.key, current.derivation.key)) {
    derivation.free();
    throw new HostFailure({ error: "superseded", held: current.derivation.key, posted: derivation.key });
  }
  current?.derivation.free();
  held.set(journey, { document, derivation });
  return JSON.stringify(derivation.key);
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
      return engine.version;
    case "load":
      return load(request.document);
    case "derived":
      return holding(request.journey).derivation.derivedText();
    case "project":
      return holding(request.journey).derivation.projectText(request.request);
    case "render_draft":
      return holding(request.journey).derivation.renderDraftText(request.request);
    case "preview":
      return ready().previewText(holding(request.journey).document, request.request);
    case "apply":
      return ready().applyText(holding(request.journey).document, request.request);
    case "release":
      held.get(request.journey)?.derivation.free();
      held.delete(request.journey);
      return "";
    case "route_level":
      return ready().routeLevelText(request.request);
    case "route_notices":
      return ready().routeNoticesText(request.request);
    case "apply_route":
      return ready().applyRouteText(request.request);
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
