// Proposals over either host (I6, C14; brief 5.7): fetched, created, edited, previewed,
// applied, discarded, and refreshed by id, and the upgrade, save-as-route, and re-link drafts
// (B7, B8, B9). The server answers over HTTP (`/api/{domain}/proposals`, `/api/proposals/{id}...`,
// `/api/journeys/{id}/{upgrade,save-as-route,relink}`); the in-browser host answers the same JSON
// from its root. Each write names a client patch id (H5) and a client proposal id (I6), so a
// lost answer is resubmitted rather than guessed at; nothing retries on its own.
import { answerFailure, networkFailure, type CairnClient, type HttpFailure, type Schema } from "@cairn/client";
import { HostFailure, type InBrowserHost } from "@cairn/wasm";

import { Missing, ReadFailed } from "./host.ts";

export type Proposal = Schema<"Proposal">;
export type ProposalDraft = Schema<"ProposalDraft">;
export type ProposalReview = Schema<"ProposalReview">;
export type ProposalAnswer = Schema<"ProposalAnswer">;
export type PatchAnswer = Schema<"PatchAnswer">;
export type Rejection = Schema<"Rejection">;
export type Domain = Schema<"Domain">;
export type Lineage = Schema<"Lineage">;

/** How a write to a proposal ended: its answer, the rejection (A15, H5), or why there is none. */
export type ProposalWritten<A> =
  | { outcome: "answered"; answer: A }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "failed"; error: HttpFailure };

/** The proposals of one host. */
export interface ProposalHost {
  /** I6: the proposal with this id; `Missing` when there is none. */
  get(id: string): Promise<Proposal>;
  /** I6: a new proposal for `destination` (which may not exist yet: revision 0). */
  create(destination: Domain, request: Schema<"ProposalCreate">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6, H5: its new content, against the editing revision its editor saw. */
  edit(id: string, request: Schema<"ProposalEdit">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6: discards it. */
  discard(id: string, request: Schema<"ProposalStep">): Promise<ProposalWritten<ProposalAnswer>>;
  /** I6: drafted again on its destination as it stands, the reviewer's choices carried over. */
  refresh(id: string, request: Schema<"ProposalStep">): Promise<ProposalWritten<ProposalAnswer>>;
  /** C14, D7, I6: what applying it now would do, and what moved since it was drafted. */
  preview(id: string): Promise<ProposalReview>;
  /** I6, H2: applies it as the caller, at the editing revision they reviewed. */
  apply(id: string, request: Schema<"ProposalApply">): Promise<ProposalWritten<PatchAnswer>>;
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

function isRejection(body: unknown): body is Rejection {
  return typeof body === "object" && body !== null && "rejection" in body;
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
  if ((response.status === 409 || response.status === 422) && isRejection(error)) {
    return { outcome: "rejected", rejection: error };
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

/** The server's proposals, through `client`. */
export function serverProposals(client: CairnClient): ProposalHost {
  const at = (id: string) => ({ params: { path: { id } } });
  return {
    get: (id) => readOver(id, () => client.GET("/api/proposals/{id}", at(id))),
    create: (destination, body) =>
      written(() => {
        if (destination === "deployment") {
          return client.POST("/api/deployment/proposals", { body });
        }
        if ("journey" in destination) {
          return client.POST("/api/journeys/{id}/proposals", { ...at(destination.journey), body });
        }
        return client.POST("/api/routes/{id}/proposals", { ...at(destination.route), body });
      }),
    edit: (id, body) => written(() => client.PATCH("/api/proposals/{id}", { ...at(id), body })),
    discard: (id, body) => written(() => client.POST("/api/proposals/{id}/discard", { ...at(id), body })),
    refresh: (id, body) => written(() => client.POST("/api/proposals/{id}/refresh", { ...at(id), body })),
    preview: (id) => readOver(id, () => client.POST("/api/proposals/{id}/preview", at(id))),
    apply: (id, body) => written(() => client.POST("/api/proposals/{id}/apply", { ...at(id), body })),
    upgrade: (journey, body) => written(() => client.POST("/api/journeys/{id}/upgrade", { ...at(journey), body })),
    saveAsRoute: (journey, body) => written(() => client.POST("/api/journeys/{id}/save-as-route", { ...at(journey), body })),
    relink: (journey, body) => written(() => client.POST("/api/journeys/{id}/relink", { ...at(journey), body })),
  };
}

/** A failure of the root as a proposal write reads one: what it says, with no HTTP status. */
function rootFailure(thrown: unknown): HttpFailure {
  if (thrown instanceof HostFailure) {
    const reason = thrown.reason;
    return { status: 0, message: "message" in reason ? reason.message : reason.error };
  }
  return { status: 0, message: thrown instanceof Error ? thrown.message : String(thrown) };
}

/** A write to the root, answered as a server's would be. */
function writtenBy<A>(call: () => A): Promise<ProposalWritten<A>> {
  try {
    return Promise.resolve({ outcome: "answered", answer: call() });
  } catch (thrown) {
    if (thrown instanceof HostFailure && thrown.reason.error === "rejected") {
      return Promise.resolve({ outcome: "rejected", rejection: thrown.reason.rejection });
    }
    return Promise.resolve({ outcome: "failed", error: rootFailure(thrown) });
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
    return Promise.reject(new ReadFailed(rootFailure(thrown)));
  }
}

/** The in-browser root's proposals. */
export function browserProposals(root: InBrowserHost): ProposalHost {
  return {
    get: (id) => readBy(id, () => root.proposal(id)),
    create: (destination, request) => writtenBy(() => root.propose(destination, request)),
    edit: (id, request) => writtenBy(() => root.editProposal(id, request)),
    discard: (id, request) => writtenBy(() => root.discardProposal(id, request)),
    refresh: (id, request) => writtenBy(() => root.refreshProposal(id, request)),
    preview: (id) => readBy(id, () => root.previewProposal(id)),
    apply: (id, request) => writtenBy(() => root.applyProposal(id, request)),
    upgrade: (journey, request) => writtenBy(() => root.proposeUpgrade(journey, request)),
    saveAsRoute: (journey, request) => writtenBy(() => root.proposeSaveAsRoute(journey, request)),
    relink: (journey, request) => writtenBy(() => root.proposeRelink(journey, request)),
  };
}
