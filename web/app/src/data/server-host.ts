// The server host: the API over HTTP at the page's origin (the binary serves the UI and the
// API on one port; Vite's dev server proxies the API's paths to a running server), signed in
// by the session cookie, with ticks over Server-Sent Events. A domain document counts only
// when the server stamps it with the build this page was loaded from (ARCHITECTURE, Web UI:
// version skew).
import {
  type Answered,
  type HttpFailure,
  answerFailure,
  createCairnClient,
  eventSourceTicks,
  networkFailure,
  rejectionOf,
  sendOver,
  type CairnClient,
  type Patch,
} from "@cairn/client";
import { serverAssistant } from "./assistant.ts";
import type { AgentToken } from "./host.ts";
import { serverProposals } from "./proposals.ts";
import { Missing, ReadFailed, Skewed, type Host, type JourneyIndexQuery, type Markdown, type RouteImport, type TourWrite, type TourWriteRequest } from "./host.ts";

interface Reply<T> {
  data?: T;
  error?: unknown;
  response: Response;
}

/** The header the server stamps every response with: the digest of the web build it serves (crates/cairn/src/assets.rs). */
const BUILD_HEADER = "cairn-build";

/** The answer of a read with its response, or why there is none. */
async function replied<T>(what: string, read: () => Promise<Reply<T>>): Promise<{ data: T; response: Response }> {
  let reply: Reply<T>;
  try {
    reply = await read();
  } catch (thrown) {
    throw new ReadFailed(networkFailure(thrown));
  }
  if (reply.data !== undefined) {
    return { data: reply.data, response: reply.response };
  }
  if (reply.response.status === 404) {
    throw new Missing(what);
  }
  throw new ReadFailed(answerFailure(reply.response.status, reply.error));
}

/** The answer of a read, or why there is none. */
async function answered<T>(what: string, read: () => Promise<Reply<T>>): Promise<T> {
  return (await replied(what, read)).data;
}

/** The journey index's query parameters (C16), as `GET /api/journeys` reads them. */
export function indexParams(query: JourneyIndexQuery = {}) {
  return {
    ...(query.status === undefined || query.status.length === 0 ? {} : { status: query.status }),
    ...(query.route === undefined ? {} : { route: query.route }),
    ...(query.version === undefined ? {} : { version: query.version }),
    ...(query.referencing === undefined || query.referencing.length === 0 ? {} : { referencing: query.referencing }),
    ...(query.owner === undefined || query.owner.length === 0 ? {} : { owner: query.owner }),
    ...(query.upgrade_available === undefined ? {} : { upgrade_available: query.upgrade_available }),
    ...(query.after === undefined ? {} : { after: query.after }),
    ...(query.size === undefined ? {} : { size: query.size }),
  };
}

/** A13: `POST /api/routes/{id}/import`, answered as a patch is: answered, rejected, or failed. */
async function importOver(client: CairnClient, request: RouteImport): Promise<Answered<HttpFailure>> {
  let reply;
  try {
    reply = await client.POST("/api/routes/{id}/import", { params: { path: { id: request.file.route } }, body: request });
  } catch (thrown) {
    return { outcome: "failed", error: networkFailure(thrown) };
  }
  const { data, error, response } = reply;
  if (data !== undefined) {
    return { outcome: "answered", answer: data };
  }
  const rejection = rejectionOf(response.status, error);
  if (rejection !== undefined) {
    return { outcome: "rejected", rejection };
  }
  return { outcome: "failed", error: answerFailure(response.status, error) };
}

/** The status and body of a call that answered no data, as a failed read. */
function refused<T>(reply: Reply<T>): ReadFailed {
  return new ReadFailed(answerFailure(reply.response.status, reply.error));
}

/** H2: the caller's agent tokens over the server's endpoints; a call that cannot be made is a failed read. */
function serverAgentTokens(client: CairnClient): NonNullable<Host["agentTokens"]> {
  const attempt = async <T>(call: () => Promise<Reply<T>>): Promise<Reply<T>> => {
    try {
      return await call();
    } catch (thrown) {
      throw new ReadFailed(networkFailure(thrown));
    }
  };
  return {
    async list() {
      const reply = await attempt(() => client.GET("/api/users/me/tokens"));
      if (reply.data === undefined) {
        throw refused(reply);
      }
      // Newest first; a revoked token is gone from the page, though the API still lists it.
      const live: AgentToken[] = reply.data.filter((token) => token.revoked_at === undefined || token.revoked_at === null);
      return live.sort((one, other) => other.created_at.localeCompare(one.created_at));
    },
    async mint(name) {
      const reply = await attempt(() => client.POST("/api/users/me/tokens", { body: { name } }));
      if (reply.data === undefined) {
        throw refused(reply);
      }
      return reply.data;
    },
    async revoke(agent) {
      const reply = await attempt(() => client.DELETE("/api/users/me/tokens/{agent}", { params: { path: { agent } } }));
      if (!reply.response.ok) {
        throw refused(reply);
      }
    },
  };
}

/** `PUT /api/users/me/tour`: written, over another record than expected (409, with the current one), or not answered. */
async function putTourOver(client: CairnClient, write: TourWriteRequest): Promise<TourWrite> {
  let reply;
  try {
    reply = await client.PUT("/api/users/me/tour", { body: write });
  } catch (thrown) {
    return { ok: false, failure: networkFailure(thrown) };
  }
  const { error, response } = reply;
  if (response.ok) {
    return { ok: true };
  }
  if (response.status === 409 && typeof error === "object" && "state" in error) {
    return { ok: false, current: error };
  }
  return { ok: false, failure: answerFailure(response.status, error) };
}

/** The server at `origin`, with the build the page was loaded from (none on a page Vite's dev server served, which is never in skew). */
export function serverHost(origin: string, build: string | undefined): Host {
  const client: CairnClient = createCairnClient(origin);
  return {
    capabilities: () => answered("the capabilities", () => client.GET("/api/capabilities")),
    journeys: (query) => answered("the journey index", () => client.GET("/api/journeys", { params: { query: indexParams(query) } })),
    routes: (after, kind) => answered("the route index", () => client.GET("/api/routes", { params: { query: { ...(after === undefined ? {} : { after }), ...(kind === undefined ? {} : { kind }) } } })),
    routeDetail: (route) => answered(route, () => client.GET("/api/routes/{id}/versions", { params: { path: { id: route } } })),
    exportRoute: (route, version) =>
      answered(version === undefined ? `${route}'s draft` : `${route} version ${String(version)}`, () =>
        client.GET("/api/routes/{id}/export", { params: { path: { id: route }, query: version === undefined ? {} : { version } } }),
      ),
    importRoute: (request) => importOver(client, request),
    caller: () => answered("the caller", () => client.GET("/api/users/me")),
    tour: () => answered("the tour", () => client.GET("/api/users/me/tour")),
    putTour: (write) => putTourOver(client, write),
    userNames: (users) => answered("the users", () => client.GET("/api/users", { params: { query: { id: [...users] } } })),
    deployment: () => answered("the deployment", () => client.GET("/api/deployment")),
    identityReferences: (identity) => answered(identity, () => client.GET("/api/deployment/identities/{key}/references", { params: { path: { key: identity } } })),
    documentText: async (journey) => {
      const { data, response } = await replied(journey, () =>
        client.GET("/api/journeys/{id}/document", { params: { path: { id: journey } }, parseAs: "text" }),
      );
      // A page with no build is one Vite's dev server served: nothing to compare.
      if (build !== undefined && response.headers.get(BUILD_HEADER) !== build) {
        throw new Skewed();
      }
      return data;
    },
    route: (route) => answered(route, () => client.GET("/api/routes/{id}", { params: { path: { id: route } } })),
    routeVersion: (route, version) =>
      answered(`${route} version ${String(version)}`, () =>
        client.GET("/api/routes/{id}/versions/{version}", { params: { path: { id: route, version } } }),
      ),
    history: (journey, node, after) =>
      answered(journey, () =>
        client.GET("/api/journeys/{id}/history", {
          params: {
            path: { id: journey },
            query: { ...(node === undefined ? {} : { node }), ...(after === undefined ? {} : { after }) },
          },
        }),
      ),
    send: (patch: Patch, note?: Markdown) => sendOver(client, note)(patch),
    proposals: serverProposals(client),
    assistant: serverAssistant(client),
    agentTokens: serverAgentTokens(client),
    openTicks: eventSourceTicks(origin),
  };
}
