// H5's safe retry, as logic that knows no transport (the TypeScript side of
// crates/api/src/client/retry.rs, with the same semantics): a host's `send` submits one
// patch, and `submit` resubmits it while each stale answer is safe to retry.
//
// A stale patch is resubmitted on its own only when what intervened cannot have touched
// anything it touches: the rejection's intervening touched set does not overlap the patch's
// own, as the engine's touched-set function says (the wasm engine, passed in as `overlaps`,
// so this package does not depend on the browser host). The resubmission names the revisions
// the rejection reports, never a newer one read since, and keeps its patch id. It only ever
// moves forward: a conflict whose current revision is not past the one the patch named is
// surfaced, never rebased onto
// (decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md).
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

/** H5: whether a stale rejection's intervening set overlaps what the patch touches. */
export type Overlaps = (patch: Patch, intervening: TouchedSet) => boolean;

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
  /** The engine's overlap check. A throw (the module refused) makes a retry unsafe. */
  overlaps: Overlaps;
  /**
   * Whether automatic retries are allowed now; asked before each resubmission. Version skew
   * stops them, since an old engine's touched sets may be wrong (ARCHITECTURE, Web UI).
   */
  mayRetry?: () => boolean;
}

/** The domain a patch targets, or none for a proposal (never rebased). */
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

function overlapsSafely(overlaps: Overlaps, patch: Patch, intervening: TouchedSet): boolean {
  try {
    return overlaps(patch, intervening);
  } catch {
    return true;
  }
}

/**
 * H5: the patch to resubmit after it was answered stale with `conflicts` and `intervening`,
 * or none when it is not safe: what intervened overlaps what it touches, or a revision
 * moved that the patch names nowhere it could be rebased, or a conflict names a current
 * revision not past the one the patch named (rebasing would move it backward).
 */
export function rebased(
  patch: Patch,
  conflicts: RevisionConflict[],
  intervening: TouchedSet,
  overlaps: Overlaps,
): Patch | undefined {
  const target = targetDomain(patch.target);
  if (target === undefined || conflicts.length === 0 || overlapsSafely(overlaps, patch, intervening)) {
    return undefined;
  }
  const next: Patch = { ...patch };
  for (const conflict of conflicts) {
    if (conflict.current <= conflict.expected || !("domain" in conflict.of)) {
      return undefined;
    }
    const moved = conflict.of.domain;
    if (sameDomain(moved, target)) {
      next.base_revision = conflict.current;
    } else if (moved === "deployment" && patch.deployment_revision != null) {
      next.deployment_revision = conflict.current;
    } else {
      return undefined;
    }
  }
  return next;
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
    const allowed = resubmitted < RESUBMISSION_COUNT_MAX && (options.mayRetry?.() ?? true);
    const next =
      allowed && rejection.rejection === "stale"
        ? rebased(current, rejection.conflicts, rejection.intervening, options.overlaps)
        : undefined;
    if (next === undefined) {
      return { outcome: "rejected", rejection, resubmitted };
    }
    current = next;
    resubmitted += 1;
  }
}
