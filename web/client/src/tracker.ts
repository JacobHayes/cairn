// H6's subscription tracking, as pure functions over a value that knows no transport (the
// TypeScript side of crates/api/src/client/tracker.rs, with the same semantics): a view holds the revision it
// fetched of each domain it shows, and a tick asks it to refetch only when the tick is newer
// than what it holds and than a refetch it already asked for. The current revisions a stream
// starts with, again on every reconnect, ask for nothing the view already has, so a view
// never misses a commit and never refetches twice for one. A refetch that fails, or a stream
// that reopens, forgets what was asked for, so the next tick (a reconnect's current revisions
// among them) asks again: a fault delays a view, never leaves it stale once it is over.
import type { components } from "../generated/api.ts";

export type Tick = components["schemas"]["Tick"];
export type RevisionOf = components["schemas"]["RevisionOf"];

/**
 * The name a domain or proposal is watched by on the stream (`deployment`, `journey:<id>`,
 * `route:<id>`, `proposal:<id>`), which also keys it here.
 */
export function watchName(of: RevisionOf): string {
  if ("proposal" in of) {
    return `proposal:${of.proposal}`;
  }
  const domain = of.domain;
  if (domain === "deployment") {
    return "deployment";
  }
  return "journey" in domain ? `journey:${domain.journey}` : `route:${domain.route}`;
}

/** What a view holds, and what it has asked to refetch, by watch name. */
export interface Tracked {
  readonly held: Readonly<Record<string, number>>;
  readonly wanted: Readonly<Record<string, number>>;
}

/** A view that holds nothing and has asked for nothing. */
export const UNTRACKED: Tracked = { held: {}, wanted: {} };

const without = (record: Readonly<Record<string, number>>, name: string): Readonly<Record<string, number>> =>
  Object.fromEntries(Object.entries(record).filter(([each]) => each !== name));

/** The view now holds `of` at `revision`, from a fetch: never older than it held. */
export function fetched(tracked: Tracked, of: RevisionOf, revision: number): Tracked {
  const name = watchName(of);
  const held = Math.max(tracked.held[name] ?? revision, revision);
  const wanted = tracked.wanted[name];
  return {
    held: { ...tracked.held, [name]: held },
    wanted: wanted !== undefined && wanted <= held ? without(tracked.wanted, name) : tracked.wanted,
  };
}

/** The refetch a tick asked for of `of` failed: the next tick at that revision asks again. */
export function refetchFailed(tracked: Tracked, of: RevisionOf): Tracked {
  return { held: tracked.held, wanted: without(tracked.wanted, watchName(of)) };
}

/**
 * The stream was opened again: every refetch asked for and not yet made is forgotten, and
 * the current revisions the new stream starts with ask for whatever is newer than held.
 */
export function reopened(tracked: Tracked): Tracked {
  return { held: tracked.held, wanted: {} };
}

/** The view no longer shows `of`: it holds nothing of it. */
export function forget(tracked: Tracked, of: RevisionOf): Tracked {
  const name = watchName(of);
  return { held: without(tracked.held, name), wanted: without(tracked.wanted, name) };
}

/** The revision the view holds of `of`, if it holds it. */
export function heldOf(tracked: Tracked, of: RevisionOf): number | undefined {
  return tracked.held[watchName(of)];
}

/**
 * H6: what the view tracks once `tick` asked it to refetch `tick.of`, or none when it asks
 * nothing: the tick is newer than what the view holds and than any refetch already asked for.
 */
export function ticked(tracked: Tracked, tick: Tick): Tracked | undefined {
  const name = watchName(tick.of);
  const known = Math.max(tracked.held[name] ?? -1, tracked.wanted[name] ?? -1);
  if (known >= tick.revision) {
    return undefined;
  }
  return { held: tracked.held, wanted: { ...tracked.wanted, [name]: tick.revision } };
}
