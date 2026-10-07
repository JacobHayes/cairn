// The server host: the API over HTTP at the page's origin (the binary serves the UI and the
// API on one port; Vite's dev server proxies the API's paths to a running server), signed in
// by the session cookie, with ticks over Server-Sent Events.
import {
  type Answered,
  type HttpFailure,
  answerFailure,
  createCairnClient,
  eventSourceTicks,
  networkFailure,
  sendOver,
  type CairnClient,
  type Patch,
} from "@cairn/client";
import type { Engine } from "@cairn/wasm";

import { Missing, ReadFailed, type Host, type JourneyIndexQuery, type Markdown, type RouteFiles, type RouteImport } from "./host.ts";

interface Reply<T> {
  data?: T;
  error?: unknown;
  response: Response;
}

/** The answer of a read, or why there is none. */
async function answered<T>(what: string, read: () => Promise<Reply<T>>): Promise<T> {
  let reply: Reply<T>;
  try {
    reply = await read();
  } catch (thrown) {
    throw new ReadFailed(networkFailure(thrown));
  }
  if (reply.data !== undefined) {
    return reply.data;
  }
  if (reply.response.status === 404) {
    throw new Missing(what);
  }
  throw new ReadFailed(answerFailure(reply.response.status, reply.error));
}

/** The page engine's route files. */
export function filesOf(engine: Engine): RouteFiles {
  return {
    read: (text) => engine.readRouteFile(text),
    text: (file) => engine.routeFileText(file),
  };
}

/** The journey index's query parameters (C16), as `GET /journeys` reads them. */
export function indexParams(query: JourneyIndexQuery = {}) {
  return {
    ...(query.status === undefined || query.status.length === 0 ? {} : { status: query.status }),
    ...(query.route === undefined ? {} : { route: query.route }),
    ...(query.version === undefined ? {} : { version: query.version }),
    ...(query.referencing === undefined || query.referencing.length === 0 ? {} : { referencing: query.referencing }),
    ...(query.upgrade_available === undefined ? {} : { upgrade_available: query.upgrade_available }),
    ...(query.after === undefined ? {} : { after: query.after }),
    ...(query.size === undefined ? {} : { size: query.size }),
  };
}

/** A13: `POST /routes/{id}/import`, answered as a patch is: answered, rejected, or failed. */
async function importOver(client: CairnClient, request: RouteImport): Promise<Answered<HttpFailure>> {
  let reply;
  try {
    reply = await client.POST("/routes/{id}/import", { params: { path: { id: request.file.route } }, body: request });
  } catch (thrown) {
    return { outcome: "failed", error: networkFailure(thrown) };
  }
  const { data, error, response } = reply;
  if (data !== undefined) {
    return { outcome: "answered", answer: data };
  }
  if ((response.status === 409 || response.status === 422) && typeof error === "object" && "rejection" in error) {
    return { outcome: "rejected", rejection: error };
  }
  return { outcome: "failed", error: answerFailure(response.status, error) };
}

/** The server at `origin`, with this page's engine for touched sets. */
export function serverHost(origin: string, engine: Engine): Host {
  const client: CairnClient = createCairnClient(origin);
  return {
    kind: "server",
    engineVersion: engine.version,
    overlaps: (patch, intervening) => engine.touchedOverlaps(patch, intervening),
    capabilities: () => answered("the capabilities", () => client.GET("/capabilities")),
    journeys: (query) => answered("the journey index", () => client.GET("/journeys", { params: { query: indexParams(query) } })),
    routes: (after) => answered("the route index", () => client.GET("/routes", { params: { query: after === undefined ? {} : { after } } })),
    routeDetail: (route) => answered(route, () => client.GET("/routes/{id}/versions", { params: { path: { id: route } } })),
    exportRoute: (route, version) =>
      answered(version === undefined ? `${route}'s draft` : `${route} version ${String(version)}`, () =>
        client.GET("/routes/{id}/export", { params: { path: { id: route }, query: version === undefined ? {} : { version } } }),
      ),
    importRoute: (request) => importOver(client, request),
    viewer: () => answered("the viewer", () => client.GET("/users/me")),
    files: filesOf(engine),
    deployment: () => answered("the deployment", () => client.GET("/deployment")),
    documentText: (journey) =>
      answered(journey, () =>
        client.GET("/journeys/{id}/document", { params: { path: { id: journey } }, parseAs: "text" }),
      ),
    route: (route) => answered(route, () => client.GET("/routes/{id}", { params: { path: { id: route } } })),
    routeVersion: (route, version) =>
      answered(`${route} version ${String(version)}`, () =>
        client.GET("/routes/{id}/versions/{version}", { params: { path: { id: route, version } } }),
      ),
    history: (journey, node, after) =>
      answered(journey, () =>
        client.GET("/journeys/{id}/history", {
          params: {
            path: { id: journey },
            query: { ...(node === undefined ? {} : { node }), ...(after === undefined ? {} : { after }) },
          },
        }),
      ),
    send: (patch: Patch, note?: Markdown) => sendOver(client, note)(patch),
    openTicks: eventSourceTicks(origin),
  };
}

/**
 * Whether a server answers at `origin`: its capabilities document comes back as JSON. A
 * static site or a dev server with no server behind its proxy answers something else.
 */
export async function serverAnswers(origin: string): Promise<boolean> {
  try {
    const response = await fetch(`${origin}/capabilities`, { headers: { Accept: "application/json" } });
    const type = response.headers.get("content-type") ?? "";
    return response.ok && type.includes("application/json");
  } catch {
    return false;
  }
}
