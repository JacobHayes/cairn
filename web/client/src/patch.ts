// The patch types and the shape of one answer to a patch, shared by the hosts (H5). A patch is
// sent once, as it is: the server answers a stale one stale, and the writer reads again and
// decides again, so no client retries on its own.
import type { components } from "../generated/api.ts";

type Schemas = components["schemas"];
export type Patch = Schemas["Patch"];
export type PatchAnswer = Schemas["PatchAnswer"];
export type ProposalAnswer = Schemas["ProposalAnswer"];
export type PatchSubmitted = Schemas["PatchSubmitted"];
export type Markdown = Schemas["Markdown"];
export type Rejection = Schemas["Rejection"];
export type RevisionConflict = Schemas["RevisionConflict"];
export type TouchedSet = Schemas["TouchedSet"];
export type Domain = Schemas["Domain"];

/** What one submission was answered: the answer (a patch's by default), a rejection, or a failure. */
export type Answered<E, A = PatchAnswer> =
  | { outcome: "answered"; answer: A }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "failed"; error: E };

/** What goes with a patch (I6): a note for its events, which titles a proposal it creates; and the proposal to write it to instead of applying it, with its editing revision when it exists. */
export interface PatchExtras {
  note?: Markdown;
  proposal?: string;
  proposal_revision?: number;
}

/** Sends one patch to a host: applied, or written to a proposal. */
export type Send<E> = (patch: Patch) => Promise<Answered<E, PatchSubmitted>>;

/** The domain a patch targets, or none for a proposal. */
export function targetDomain(target: Patch["target"]): Domain | undefined {
  if (target === "deployment") {
    return "deployment";
  }
  if ("journey" in target) {
    return { journey: target.journey };
  }
  if ("route" in target) {
    return { route: target.route };
  }
  return undefined;
}

/** Whether two domains are the same one. */
export function sameDomain(left: Domain, right: Domain): boolean {
  if (left === "deployment" || right === "deployment") {
    return left === right;
  }
  if ("journey" in left) {
    return "journey" in right && left.journey === right.journey;
  }
  return "route" in right && left.route === right.route;
}
