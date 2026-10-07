// The in-browser host (ARCHITECTURE, Web UI: in-browser host): the same service over the
// memory store seeded with the fixtures, in this page (4.5's InBrowserHost), answering the
// API's JSON. Nothing persists and each tab is its own; its ticks come from the in-process
// notifier, so its stream never drops.
import type { Answered, HttpFailure, OpenTicks } from "@cairn/client";
import { HostFailure, type InBrowserHost } from "@cairn/wasm";

import { Missing, ReadFailed, type Host, type Markdown, type Patch, type RouteImport } from "./host.ts";
import { browserProposals } from "./proposals.ts";
import { filesOf } from "./server-host.ts";

/** A failure of the root as the data layer reads one: what it says, with no HTTP status. */
function failure(thrown: unknown): HttpFailure {
  if (thrown instanceof HostFailure) {
    const reason = thrown.reason;
    const message = "message" in reason ? reason.message : reason.error;
    return { status: 0, message };
  }
  return { status: 0, message: thrown instanceof Error ? thrown.message : String(thrown) };
}

/** A read of the root, in the shape a server read takes. */
function read<T>(what: string, call: () => T): Promise<T> {
  try {
    return Promise.resolve(call());
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "missing") {
      return Promise.reject(new Missing(what));
    }
    return Promise.reject(new ReadFailed(failure(thrown)));
  }
}

function send(root: InBrowserHost, patch: Patch, note?: Markdown): Promise<Answered<HttpFailure>> {
  try {
    const answer = root.patch(note === undefined ? { patch } : { patch, note });
    return Promise.resolve({ outcome: "answered", answer });
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
      return Promise.resolve({ outcome: "rejected", rejection: thrown.reason.rejection });
    }
    return Promise.resolve({ outcome: "failed", error: failure(thrown) });
  }
}

/** A13: an import as the root answers it, in the shape a server's answer takes. */
function importInto(root: InBrowserHost, request: RouteImport): Promise<Answered<HttpFailure>> {
  try {
    return Promise.resolve({ outcome: "answered", answer: root.importFile(request) });
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
      return Promise.resolve({ outcome: "rejected", rejection: thrown.reason.rejection });
    }
    return Promise.resolve({ outcome: "failed", error: failure(thrown) });
  }
}

/** The root's subscriptions as a tick stream: its first take is the current revisions. */
function ticksOf(root: InBrowserHost): OpenTicks {
  return (watching, handlers) =>
    root.subscribe([...watching], (ticks, first) => {
      if (first) {
        handlers.opened();
      }
      for (const tick of ticks) {
        handlers.tick(tick);
      }
    });
}

/** The in-browser host over `root`. */
export function browserHost(root: InBrowserHost): Host {
  const engine = root.engine;
  return {
    kind: "browser",
    engineVersion: engine.version,
    overlaps: (patch, intervening) => engine.touchedOverlaps(patch, intervening),
    capabilities: () => read("the capabilities", () => root.capabilities()),
    journeys: (query) => read("the journey index", () => root.journeyIndex(query ?? {})),
    routes: (after) => read("the route index", () => root.routes(after)),
    routeDetail: (route) => read(route, () => root.routeDetail(route)),
    exportRoute: (route, version) =>
      read(version === undefined ? `${route}'s draft` : `${route} version ${String(version)}`, () => root.exportFile(route, version)),
    importRoute: (request) => importInto(root, request),
    viewer: () => read("the viewer", () => root.viewer()),
    files: filesOf(engine),
    deployment: () => read("the deployment", () => root.deployment()),
    documentText: (journey) => read(journey, () => root.documentText(journey)),
    route: (route) => read(route, () => root.route(route)),
    routeVersion: (route, version) => read(`${route} version ${String(version)}`, () => root.routeVersion(route, version)),
    history: (journey, node, after) => read(journey, () => root.history(journey, node, after)),
    send: (patch, note) => send(root, patch, note),
    proposals: browserProposals(root),
    openTicks: ticksOf(root),
  };
}
