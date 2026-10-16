// One read kept current while a screen shows it (H6): the index screens' reads (a filtered
// page of journeys, the route index, a route's detail) each watch what they show, refetch
// when a tick is newer than the revision they hold (or names something they do not hold, or
// deletes something they do), and never show an older answer over a newer one. A failed fetch
// forgets what was asked for, so the next tick asks again; a read that once answered keeps its
// answer through a failure and is tried again. It is a keyed read (keyed-read.ts) whose source
// is the spec: what to fetch, what to watch, what the answer holds. Journeys themselves are
// derived by the journey manager (journeys.ts).
import type { RevisionOf } from "@cairn/client";

import { HOST_READ, type ReadSource } from "./keyed-read.ts";

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

const LIVE: ReadSource<{ spec: LiveSpec<unknown> }, unknown> = {
  policy: HOST_READ,
  read: ({ spec }) => spec.fetch(),
  // A stream opened again lists what exists: a domain deleted while it was down is simply absent, so a read that has answered asks again.
  ticks: ({ spec }) => ({ watching: spec.watching, about: spec.about, holds: spec.holds, rereadOnReopen: true }),
};

/** The source of a live read of `T`: its args are the spec. */
export const liveSource = <T>(): ReadSource<{ spec: LiveSpec<T> }, T> => LIVE as unknown as ReadSource<{ spec: LiveSpec<T> }, T>;

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
