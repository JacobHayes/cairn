// The patch types and the shape of one answer to a patch, shared by the hosts (H5). A patch is
// sent once, as it is: the server decides a stale one, applying it past what intervened when
// that is safe and answering it stale otherwise, so no client retries on its own.
import type { components } from "../generated/api.ts";

type Schemas = components["schemas"];
export type Patch = Schemas["Patch"];
export type PatchAnswer = Schemas["PatchAnswer"];
export type Rejection = Schemas["Rejection"];
export type RevisionConflict = Schemas["RevisionConflict"];
export type TouchedSet = Schemas["TouchedSet"];
export type Domain = Schemas["Domain"];

/** What one submission was answered: the patch answer, a rejection, or a failure. */
export type Answered<E> =
  | { outcome: "answered"; answer: PatchAnswer }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "failed"; error: E };

/** Sends one patch to a host. */
export type Send<E> = (patch: Patch) => Promise<Answered<E>>;

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
