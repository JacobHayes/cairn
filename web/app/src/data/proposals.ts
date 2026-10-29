// Proposals over either host (I6, C14): fetched, created, edited, previewed,
// applied, discarded, and refreshed by id, and the upgrade, save-as-route, and re-link drafts
// (B7, B8, B9). The server answers over HTTP (`/api/{domain}/proposals`, `/api/proposals/{id}...`,
// `/api/journeys/{id}/{upgrade,save-as-route,relink}`); the in-browser host answers the same JSON
// from its root. Each write names a client patch id (H5) and a client proposal id (I6), so a
// lost answer is resubmitted rather than guessed at; nothing retries on its own.
import { answerFailure, networkFailure, rejectionOf, type CairnClient, type HttpFailure, type Schema } from "@cairn/client";
import { HostFailure, type InBrowserHost } from "@cairn/wasm";

import { hostFailure } from "./host-failure.ts";
import { Missing, ReadFailed } from "./host.ts";

export type Proposal = Schema<"Proposal">;
export type ProposalReview = Schema<"ProposalReview">;
export type ProposalAnswer = Schema<"ProposalAnswer">;
export type PatchAnswer = Schema<"PatchAnswer">;
export type Rejection = Schema<"Rejection">;
export type Domain = Schema<"Domain">;
export type ProposalList = Schema<"ProposalList">;
export type ProposalSummary = Schema<"ProposalSummary">;
export type Cursor = Schema<"ProposalCursor">;
/** The open proposals no one destination holds (C14, C16). */
export type Listing = Schema<"ProposalListing">;

/** How a write to a proposal ended: its answer, the rejection (A15, H5), or why there is none. */
export type ProposalWritten<A> =
  | { outcome: "answered"; answer: A }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "failed"; error: HttpFailure };

/** The proposals of one host. */
export interface ProposalHost {
  /** I6: the proposal with this id under `destination`; `Missing` when there is none there. */
  get(destination: Domain, id: string): Promise<Proposal>;
  /** C14: a page of the open proposals under `destination`, newest first, with their count. */
  list(destination: Domain, after?: Cursor): Promise<ProposalList>;
  /** C14, C16: a page of the open proposals no one destination holds. */
  listing(scope: Listing, after?: Cursor): Promise<ProposalList>;
  /** I6: a new proposal for `destination` (which may not exist yet: revision 0). */
  create(destination: Domain, request: Schema<"ProposalCreate">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6, H5: its new content, against the editing revision its editor saw. */
  edit(destination: Domain, id: string, request: Schema<"ProposalEdit">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6: discards it. */
  discard(destination: Domain, id: string, request: Schema<"ProposalStep">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6: drafted again on its destination as it stands, the reviewer's choices carried over. */
  refresh(destination: Domain, id: string, request: Schema<"ProposalStep">): Promise<ProposalWritten<ProposalAnswer>>;
  /** C14, D7, I6: what applying it now would do, and what moved since it was drafted. */
  preview(destination: Domain, id: string): Promise<ProposalReview>;
  /** I6, H2: applies it as the caller, at the editing revision they reviewed. */
  apply(destination: Domain, id: string, request: Schema<"ProposalApply">): Promise<ProposalWritten<PatchAnswer>>;
  /** B7: proposes upgrading `journey`. */
  upgrade(journey: string, request: Schema<"UpgradeRequest">): Promise<ProposalWritten<ProposalAnswer>>;
  /** B8: proposes saving `journey`'s structure as a draft of a route. */
  saveAsRoute(journey: string, request: Schema<"SaveAsRouteRequest">): Promise<ProposalWritten<ProposalAnswer>>;
  /** B9: proposes re-linking `journey` to a published version. */
  relink(journey: string, request: Schema<"RelinkRequest">): Promise<ProposalWritten<ProposalAnswer>>;
}

/** A fresh proposal id (I6): `pr_` and 32 hex digits from the browser's random UUID. */
export function newProposalId(): string {
  return `pr_${crypto.randomUUID().replaceAll("-", "")}`;
}

/** The proposal a write answered, or none when it was answered from an earlier receipt. */
export function proposalOf(answer: ProposalAnswer): Proposal | undefined {
  return answer.outcome === "already_saved" ? undefined : answer.proposal;
}

interface Reply<T> {
  data?: T;
  error?: unknown;
  response: Response;
}

/** A write over HTTP, answered as a proposal write: answered, rejected, or failed. */
async function written<A>(call: () => Promise<Reply<A>>): Promise<ProposalWritten<A>> {
  let reply: Reply<A>;
  try {
    reply = await call();
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

/** A read over HTTP: its answer, `Missing` for a 404, or why there is none. */
async function readOver<T>(what: string, call: () => Promise<Reply<T>>): Promise<T> {
  let reply: Reply<T>;
  try {
    reply = await call();
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

/** Which of three calls a destination takes: the deployment's, a journey's (by id), or a route's (by id). */
function byDestination<T>(destination: Domain, calls: { deployment: () => T; journey: (id: string) => T; route: (id: string) => T }): T {
  if (destination === "deployment") {
    return calls.deployment();
  }
  return "journey" in destination ? calls.journey(destination.journey) : calls.route(destination.route);
}

const at = (id: string) => ({ params: { path: { id } } });
const under = (id: string, proposal: string) => ({ params: { path: { id, proposal } } });
const held = (proposal: string) => ({ params: { path: { proposal } } });

/** The reads of the server's proposals, each under its destination's address. */
function serverReads(client: CairnClient): Pick<ProposalHost, "get" | "list" | "listing" | "preview"> {
  const paged = (after: Cursor | undefined) => (after === undefined ? {} : { after_at: after.created_at, after: after.id });
  return {
    get: (destination, id) =>
      readOver(id, () =>
        byDestination(destination, {
          deployment: () => client.GET("/api/deployment/proposals/{proposal}", held(id)),
          journey: (journey) => client.GET("/api/journeys/{id}/proposals/{proposal}", under(journey, id)),
          route: (route) => client.GET("/api/routes/{id}/proposals/{proposal}", under(route, id)),
        }),
      ),
    list: (destination, after) =>
      readOver("proposals", () =>
        byDestination(destination, {
          deployment: () => client.GET("/api/deployment/proposals", { params: { query: paged(after) } }),
          journey: (journey) => client.GET("/api/journeys/{id}/proposals", { params: { path: { id: journey }, query: paged(after) } }),
          route: (route) => client.GET("/api/routes/{id}/proposals", { params: { path: { id: route }, query: paged(after) } }),
        }),
      ),
    listing: (scope, after) => readOver("proposals", () => client.GET("/api/proposals", { params: { query: { scope, ...paged(after) } } })),
    preview: (destination, id) =>
      readOver(id, () =>
        byDestination(destination, {
          deployment: () => client.POST("/api/deployment/proposals/{proposal}/preview", held(id)),
          journey: (journey) => client.POST("/api/journeys/{id}/proposals/{proposal}/preview", under(journey, id)),
          route: (route) => client.POST("/api/routes/{id}/proposals/{proposal}/preview", under(route, id)),
        }),
      ),
  };
}

/** The writes of the server's proposals, each under its destination's address. */
function serverWrites(client: CairnClient): Omit<ProposalHost, "get" | "list" | "listing" | "preview"> {
  return {
    create: (destination, body) =>
      written(() =>
        byDestination(destination, {
          deployment: () => client.POST("/api/deployment/proposals", { body }),
          journey: (journey) => client.POST("/api/journeys/{id}/proposals", { ...at(journey), body }),
          route: (route) => client.POST("/api/routes/{id}/proposals", { ...at(route), body }),
        }),
      ),
    edit: (destination, id, body) =>
      written(() =>
        byDestination(destination, {
          deployment: () => client.PATCH("/api/deployment/proposals/{proposal}", { ...held(id), body }),
          journey: (journey) => client.PATCH("/api/journeys/{id}/proposals/{proposal}", { ...under(journey, id), body }),
          route: (route) => client.PATCH("/api/routes/{id}/proposals/{proposal}", { ...under(route, id), body }),
        }),
      ),
    discard: (destination, id, body) =>
      written(() =>
        byDestination(destination, {
          deployment: () => client.POST("/api/deployment/proposals/{proposal}/discard", { ...held(id), body }),
          journey: (journey) => client.POST("/api/journeys/{id}/proposals/{proposal}/discard", { ...under(journey, id), body }),
          route: (route) => client.POST("/api/routes/{id}/proposals/{proposal}/discard", { ...under(route, id), body }),
        }),
      ),
    refresh: (destination, id, body) =>
      written(() =>
        byDestination(destination, {
          deployment: () => client.POST("/api/deployment/proposals/{proposal}/refresh", { ...held(id), body }),
          journey: (journey) => client.POST("/api/journeys/{id}/proposals/{proposal}/refresh", { ...under(journey, id), body }),
          route: (route) => client.POST("/api/routes/{id}/proposals/{proposal}/refresh", { ...under(route, id), body }),
        }),
      ),
    apply: (destination, id, body) =>
      written(() =>
        byDestination(destination, {
          deployment: () => client.POST("/api/deployment/proposals/{proposal}/apply", { ...held(id), body }),
          journey: (journey) => client.POST("/api/journeys/{id}/proposals/{proposal}/apply", { ...under(journey, id), body }),
          route: (route) => client.POST("/api/routes/{id}/proposals/{proposal}/apply", { ...under(route, id), body }),
        }),
      ),
    upgrade: (journey, body) => written(() => client.POST("/api/journeys/{id}/upgrade", { ...at(journey), body })),
    saveAsRoute: (journey, body) => written(() => client.POST("/api/journeys/{id}/save-as-route", { ...at(journey), body })),
    relink: (journey, body) => written(() => client.POST("/api/journeys/{id}/relink", { ...at(journey), body })),
  };
}

/** The server's proposals, through `client`. */
export function serverProposals(client: CairnClient): ProposalHost {
  return { ...serverReads(client), ...serverWrites(client) };
}

/** A write to the root, answered as a server's would be. */
function writtenBy<A>(call: () => A): Promise<ProposalWritten<A>> {
  try {
    return Promise.resolve({ outcome: "answered", answer: call() });
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
      return Promise.resolve({ outcome: "rejected", rejection: thrown.reason.rejection });
    }
    return Promise.resolve({ outcome: "failed", error: hostFailure(thrown) });
  }
}

/** A read of the root, in the shape a server read takes. */
function readBy<T>(what: string, call: () => T): Promise<T> {
  try {
    return Promise.resolve(call());
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "missing") {
      return Promise.reject(new Missing(what));
    }
    return Promise.reject(new ReadFailed(hostFailure(thrown)));
  }
}

/** The in-browser root's proposals. */
export function browserProposals(root: InBrowserHost): ProposalHost {
  return {
    get: (destination, id) => readBy(id, () => root.proposal(destination, id)),
    list: (destination, after) => readBy("proposals", () => root.listProposals({ destination }, after)),
    listing: (scope, after) => readBy("proposals", () => root.listProposals(scope, after)),
    create: (destination, request) => writtenBy(() => root.propose(destination, request)),
    edit: (destination, id, request) => writtenBy(() => root.editProposal(destination, id, request)),
    discard: (destination, id, request) => writtenBy(() => root.discardProposal(destination, id, request)),
    refresh: (destination, id, request) => writtenBy(() => root.refreshProposal(destination, id, request)),
    preview: (destination, id) => readBy(id, () => root.previewProposal(destination, id)),
    apply: (destination, id, request) => writtenBy(() => root.applyProposal(destination, id, request)),
    upgrade: (journey, request) => writtenBy(() => root.proposeUpgrade(journey, request)),
    saveAsRoute: (journey, request) => writtenBy(() => root.proposeSaveAsRoute(journey, request)),
    relink: (journey, request) => writtenBy(() => root.proposeRelink(journey, request)),
  };
}
