// H5's safe retry, as a loop that knows no transport: a host's `send` submits one patch, and
// `submit` resubmits it while each stale answer is safe to retry. Whether it is, and as what
// patch, is the engine's one rule (the schema's `Patch::rebased`, which the Rust clients call
// directly), asked through `rebase` (the derive worker's), so this package neither depends on
// the browser host nor keeps a second copy of the rule.
import type { components } from "../generated/api.ts";

type Schemas = components["schemas"];
export type Patch = Schemas["Patch"];
export type PatchAnswer = Schemas["PatchAnswer"];
export type Rejection = Schemas["Rejection"];
export type RevisionConflict = Schemas["RevisionConflict"];
export type TouchedSet = Schemas["TouchedSet"];
export type Domain = Schemas["Domain"];

/**
 * Automatic resubmissions of one patch, at most: a runaway stop, not a budget. The bound
 * is the Rust client's (`RESUBMISSION_COUNT_MAX`,
 * decisions/2026-10-06-what-the-clients-safe-retry-resubmits-and-how-often.md), so both
 * clients give up at the same point.
 */
export const RESUBMISSION_COUNT_MAX = 32;

/** A stale rejection's parts. */
export type Stale = Extract<Rejection, { rejection: "stale" }>;

/**
 * H5: the patch to resubmit after `patch` was answered `stale`, or none when a resubmission
 * is not safe. A rejection makes it unsafe.
 */
export type Rebase = (patch: Patch, stale: Stale) => Promise<Patch | undefined>;

/** What one submission was answered: the patch answer, a rejection, or a failure. */
export type Answered<E> =
  | { outcome: "answered"; answer: PatchAnswer }
  | { outcome: "rejected"; rejection: Rejection }
  | { outcome: "failed"; error: E };

/** How a submission ended, with the automatic resubmissions it took. */
export type Submission<E> =
  | { outcome: "landed"; answer: PatchAnswer; resubmitted: number }
  | { outcome: "rejected"; rejection: Rejection; resubmitted: number }
  | { outcome: "failed"; error: E; resubmitted: number };

/** Sends one patch to a host. */
export type Send<E> = (patch: Patch) => Promise<Answered<E>>;

export interface RetryOptions {
  /** The engine's safe retry. A rejection (the module refused) makes a retry unsafe. */
  rebase: Rebase;
  /**
   * Whether automatic retries are allowed now; asked before each resubmission. Version skew
   * stops them, since an old engine's safe retry may be wrong (ARCHITECTURE, Web UI).
   */
  mayRetry?: () => boolean;
}

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

/**
 * H5: submits `patch` through `send`, resubmitting it rebased while each stale answer is
 * safe to retry, at most `RESUBMISSION_COUNT_MAX` times; past that, or when a retry is not
 * safe or not allowed, the last stale rejection is surfaced.
 */
export async function submit<E>(patch: Patch, send: Send<E>, options: RetryOptions): Promise<Submission<E>> {
  let current = patch;
  let resubmitted = 0;
  for (;;) {
    const answered = await send(current);
    if (answered.outcome === "answered") {
      return { outcome: "landed", answer: answered.answer, resubmitted };
    }
    if (answered.outcome === "failed") {
      return { outcome: "failed", error: answered.error, resubmitted };
    }
    const { rejection } = answered;
    const allowed = () => resubmitted < RESUBMISSION_COUNT_MAX && (options.mayRetry?.() ?? true);
    const next =
      allowed() && rejection.rejection === "stale" ? await options.rebase(current, rejection).catch(() => undefined) : undefined;
    // Asked again once the engine answered: skew that began while it was asked stops the resend too.
    if (next === undefined || !allowed()) {
      return { outcome: "rejected", rejection, resubmitted };
    }
    current = next;
    resubmitted += 1;
  }
}
