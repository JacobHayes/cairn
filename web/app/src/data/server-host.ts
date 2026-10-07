// The server host: the API over HTTP at the page's origin (the binary serves the UI and the
// API on one port; Vite's dev server proxies the API's paths to a running server), signed in
// by the session cookie, with ticks over Server-Sent Events.
import {
  answerFailure,
  createCairnClient,
  eventSourceTicks,
  networkFailure,
  sendOver,
  type CairnClient,
  type Patch,
} from "@cairn/client";
import type { Engine } from "@cairn/wasm";

import { Missing, ReadFailed, type Host, type Markdown } from "./host.ts";

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

/** The server at `origin`, with this page's engine for touched sets. */
export function serverHost(origin: string, engine: Engine): Host {
  const client: CairnClient = createCairnClient(origin);
  return {
    kind: "server",
    engineVersion: engine.version,
    overlaps: (patch, intervening) => engine.touchedOverlaps(patch, intervening),
    capabilities: () => answered("the capabilities", () => client.GET("/capabilities")),
    journeys: () => answered("the journey index", () => client.GET("/journeys")),
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
