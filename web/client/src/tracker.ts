// H6's subscription tracking, as logic that knows no transport (the TypeScript side of
// crates/api/src/client/tracker.rs, with the same semantics): a view holds the revision it
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
export class Tracker {
  readonly #held = new Map<string, number>();
  readonly #wanted = new Map<string, number>();

  /** The view now holds `of` at `revision`, from a fetch: never older than it held. */
  fetched(of: RevisionOf, revision: number): void {
    const name = watchName(of);
    const held = Math.max(this.#held.get(name) ?? revision, revision);
    this.#held.set(name, held);
    const wanted = this.#wanted.get(name);
    if (wanted !== undefined && wanted <= held) {
      this.#wanted.delete(name);
    }
  }

  /** The refetch a tick asked for of `of` failed: the next tick at that revision asks again. */
  refetchFailed(of: RevisionOf): void {
    this.#wanted.delete(watchName(of));
  }

  /**
   * The stream was opened again: every refetch asked for and not yet made is forgotten, and
   * the current revisions the new stream starts with ask for whatever is newer than held.
   */
  reopened(): void {
    this.#wanted.clear();
  }

  /** The view no longer shows `of`: it holds nothing of it. */
  forget(of: RevisionOf): void {
    const name = watchName(of);
    this.#held.delete(name);
    this.#wanted.delete(name);
  }

  /** The revision the view holds of `of`, if it holds it. */
  held(of: RevisionOf): number | undefined {
    return this.#held.get(watchName(of));
  }

  /**
   * H6: whether `tick` asks the view to refetch `tick.of`: it is newer than what the view
   * holds and than any refetch already asked for.
   */
  ticked(tick: Tick): boolean {
    const name = watchName(tick.of);
    const known = Math.max(this.#held.get(name) ?? -1, this.#wanted.get(name) ?? -1);
    if (known >= tick.revision) {
      return false;
    }
    this.#wanted.set(name, tick.revision);
    return true;
  }
}
