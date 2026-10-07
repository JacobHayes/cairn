// One read kept current while a screen shows it (H6): the index screens' reads (a filtered
// page of journeys, the route index, a route's detail) each watch what they show, refetch
// when a tick is newer than the revision they hold (or names something they do not hold, or
// deletes something they do), and never show an older answer over a newer one. A failed
// fetch forgets what was asked for, so the next tick asks again; a read that once answered
// keeps its answer through a failure. Journeys themselves are derived by the journey store
// (journeys.ts).
import { Tracker, watchName, type RevisionOf, type Subscription, type Tick } from "@cairn/client";

import { Emitter } from "./emitter.ts";
import { Missing } from "./host.ts";

export type LiveView<T> =
  | { status: "loading" }
  | { status: "missing" }
  | { status: "failed"; message: string }
  | { status: "ready"; value: T };

/** What a live read shows and how it keeps it current. */
export interface LiveSpec<T> {
  /** The watch names the stream follows while it is shown (`journeys`, `route:<id>`...). */
  watching: string[];
  /** Whether a tick's domain is one this read shows. */
  about: (of: RevisionOf) => boolean;
  fetch: () => Promise<T>;
  /** The revisions the answer holds, so a tick at or below one asks for nothing. */
  holds: (value: T) => { of: RevisionOf; revision: number }[];
}

export class LiveRead<T> extends Emitter {
  readonly #subscription: Subscription;
  readonly #spec: LiveSpec<T>;
  readonly #tracker = new Tracker();
  #view: LiveView<T> = { status: "loading" };
  #dirty = false;
  #running = false;
  #mounted = false;
  /** The ticks that asked for a refetch not yet made, by what they name. */
  readonly #asked = new Map<string, Tick>();

  constructor(subscription: Subscription, spec: LiveSpec<T>) {
    super();
    this.#subscription = subscription;
    this.#spec = spec;
  }

  get view(): LiveView<T> {
    return this.#view;
  }

  /** Shows the read until the returned function is called: watched, fetched, kept current. */
  mount(): () => void {
    this.#mounted = true;
    const unlisten = this.#subscription.listen({
      opened: () => {
        this.#tracker.reopened();
        // A stream opened again lists what exists: a domain deleted while it was down is
        // simply absent, so a read that has answered asks again.
        if (this.#view.status === "ready") {
          this.refetch();
        }
      },
      tick: (tick) => {
        if (!this.#spec.about(tick.of)) {
          return;
        }
        if (tick.revision === 0 && this.#tracker.held(tick.of) !== undefined) {
          // Deleted (A19): revision 0 is older than anything held, yet the read is out of date.
          this.#tracker.forget(tick.of);
          this.refetch();
        } else if (this.#tracker.ticked(tick)) {
          this.#asked.set(watchName(tick.of), tick);
          this.refetch();
        }
      },
    });
    const unwatch = this.#subscription.watch(this.#spec.watching);
    this.refetch();
    return () => {
      this.#mounted = false;
      unwatch();
      unlisten();
    };
  }

  /** Fetches again: after a write this screen made, or a tick newer than held. */
  refetch(): void {
    this.#dirty = true;
    if (!this.#running) {
      void this.#run();
    }
  }

  async #run(): Promise<void> {
    this.#running = true;
    while (this.#dirty && this.#mounted) {
      this.#dirty = false;
      const asked = [...this.#asked.values()];
      this.#asked.clear();
      try {
        const value = await this.#spec.fetch();
        this.#tracker.reopened();
        // A tick that asked is answered by this fetch: what it named is now held at its
        // revision (a journey outside the filter, a route, the deployment), so the same
        // revision never asks again; the answer's own revisions follow.
        for (const tick of asked) {
          this.#tracker.fetched(tick.of, tick.revision);
        }
        for (const { of, revision } of this.#spec.holds(value)) {
          this.#tracker.fetched(of, revision);
        }
        this.#view = { status: "ready", value };
      } catch (thrown) {
        this.#tracker.reopened();
        if (thrown instanceof Missing) {
          this.#view = { status: "missing" };
        } else if (this.#view.status !== "ready") {
          this.#view = { status: "failed", message: thrown instanceof Error ? thrown.message : String(thrown) };
        }
      }
      this.emit();
    }
    this.#running = false;
  }
}

/** Every page of a paged read, in order: `read(after)` answers a page and where the next starts. */
export async function allPages<T, C>(read: (after: C | undefined) => Promise<{ items: T[]; next?: C | null }>): Promise<T[]> {
  const items: T[] = [];
  let after: C | undefined;
  for (;;) {
    const page = await read(after);
    items.push(...page.items);
    if (page.next == null) {
      return items;
    }
    after = page.next;
  }
}
