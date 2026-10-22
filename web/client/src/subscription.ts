// H6's subscription: one tick stream per page, watching the union of what its views watch,
// reopened when that union changes and again after a drop. It is one machine (ARCHITECTURE,
// Web UI): the stream is an invoked callback over the transport, the wait before reopening a
// closed stream is a delayed transition, and a batch of watch changes opens one stream (the
// reconcile it raises). Views watch with their own actor ref and are sent every tick and
// every open; each compares a tick with the revision it holds (tracker.ts), and the stream's
// opening is when each forgets what it asked for. The transport is Server-Sent Events from the
// API (`eventSourceTicks`) or the in-browser host's subscriptions, behind `OpenTicks`.
import { assign, enqueueActions, fromCallback, setup, type ActorRefFrom, type AnyActorRef, type SnapshotFrom } from "xstate";

import type { Tick } from "./tracker.ts";

/** What a tick stream tells whoever opened it. */
export interface TickHandlers {
  /** The stream (re)opened: the ticks that follow start with the current revisions. */
  opened(): void;
  tick(tick: Tick): void;
  /** The stream dropped. `closed`: it will not come back by itself. */
  dropped(closed: boolean): void;
}

/** Opens a stream watching `watching`; answers a function that closes it. */
export type OpenTicks = (watching: readonly string[], handlers: TickHandlers) => () => void;

export type StreamStatus = "idle" | "connecting" | "live" | "reconnecting";

/**
 * The first wait before reopening a stream that closed, and the longest: PRACTICES' SSE
 * coalescing interval and SSE write stall, the stream's own named limits, so a closed
 * stream is retried no faster than ticks come and no slower than a stalled one is cut.
 */
export const REOPEN_DELAY_FIRST_MS = 250;
export const REOPEN_DELAY_MAX_MS = 15_000;

/**
 * The wait before retry number `attempt` (counted from 0) of anything that backs off: the
 * first wait, doubling, at most the longest. The stream's reopening, the reads' retries
 * and a write's resends share it, so there is one backoff.
 */
export function backoffDelayMs(attempt: number): number {
  return Math.min(REOPEN_DELAY_FIRST_MS * 2 ** Math.min(attempt, 16), REOPEN_DELAY_MAX_MS);
}

/** What a watcher is sent: the stream opened (its first ticks are the current revisions), and each tick. */
export type StreamEvent = { type: "stream.opened" } | { type: "stream.tick"; tick: Tick };

/** What the subscription is asked: to follow `names` for the watcher `id`, to stop, or to reopen now. */
export type SubscriptionEvent =
  | { type: "watch"; id: string; names: readonly string[]; to: AnyActorRef }
  | { type: "unwatch"; id: string }
  /** Reopens the stream now, so its current revisions ask every view again (a refetch failed). */
  | { type: "reopen" }
  /**
   * A commit the page made, from its receipt: sent to every watcher as the stream's tick for it
   * would be, so the views fetch it at once; the stream's own tick then asks for nothing more.
   */
  | { type: "heard"; tick: Tick };

/** What the stream callback sends back, and the reconcile the subscription raises for itself. */
type Internal = StreamEvent | { type: "stream.dropped"; closed: boolean } | { type: "reconcile" };

interface Watcher {
  names: readonly string[];
  to: AnyActorRef;
}

export interface SubscriptionContext {
  open: OpenTicks;
  watchers: Readonly<Record<string, Watcher>>;
  /** What the stream now open (or being waited for) was opened with: the sorted union of the watchers' names. */
  watching: readonly string[];
  /** Closed streams in a row since one last opened; the wait before the next reopening grows with it. */
  attempt: number;
}

const namesOf = (watchers: SubscriptionContext["watchers"]): string[] =>
  [...new Set(Object.values(watchers).flatMap((watcher) => watcher.names))].sort();

const without = <T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> =>
  Object.fromEntries(Object.entries(record).filter(([each]) => each !== key));

export const subscription = setup({
  types: {
    context: {} as SubscriptionContext,
    events: {} as SubscriptionEvent | Internal,
    input: {} as { open: OpenTicks },
  },
  actors: {
    // A transport that refuses to open (the in-browser host throws for a name it cannot watch)
    // is a closed stream: tried again after the wait, never an uncaught error. Stopping the
    // callback closes the transport, and what it sends after that is dropped.
    stream: fromCallback<Internal, { open: OpenTicks; watching: readonly string[] }>(({ sendBack, input }) => {
      try {
        return input.open(input.watching, {
          opened: () => {
            sendBack({ type: "stream.opened" });
          },
          tick: (tick) => {
            sendBack({ type: "stream.tick", tick });
          },
          dropped: (closed) => {
            sendBack({ type: "stream.dropped", closed });
          },
        });
      } catch {
        sendBack({ type: "stream.dropped", closed: true });
        return () => undefined;
      }
    }),
  },
  delays: { reopen: ({ context }) => backoffDelayMs(context.attempt) },
  guards: {
    watched: ({ context }) => namesOf(context.watchers).length > 0,
    unwatched: ({ context }) => namesOf(context.watchers).length === 0,
    // The union is not what the stream was opened with.
    moved: ({ context }) => namesOf(context.watchers).join("\n") !== context.watching.join("\n"),
    closed: ({ event }) => event.type === "stream.dropped" && event.closed,
  },
  actions: {
    watch: assign(({ context, event }) => {
      const sent = event as Extract<SubscriptionEvent, { type: "watch" }>;
      return { watchers: { ...context.watchers, [sent.id]: { names: sent.names, to: sent.to } } };
    }),
    unwatch: assign(({ context, event }) => ({ watchers: without(context.watchers, (event as Extract<SubscriptionEvent, { type: "unwatch" }>).id) })),
    // A batch of changes in one turn opens one stream.
    reconcileSoon: enqueueActions(({ enqueue }) => {
      enqueue.cancel("reconcile");
      enqueue.raise({ type: "reconcile" }, { delay: 0, id: "reconcile" });
    }),
    opening: assign(({ context }) => ({ watching: namesOf(context.watchers) })),
    reset: assign({ attempt: 0 }),
    waited: assign({ attempt: ({ context }) => context.attempt + 1 }),
    broadcast: enqueueActions(({ context, event, enqueue }) => {
      for (const { to } of Object.values(context.watchers)) {
        enqueue.sendTo(to, event);
      }
    }),
    echo: enqueueActions(({ context, event, enqueue }) => {
      const { tick } = event as Extract<SubscriptionEvent, { type: "heard" }>;
      for (const { to } of Object.values(context.watchers)) {
        enqueue.sendTo(to, { type: "stream.tick", tick } satisfies StreamEvent);
      }
    }),
  },
}).createMachine({
  id: "subscription",
  context: ({ input }) => ({ open: input.open, watchers: {}, watching: [], attempt: 0 }),
  initial: "closed",
  on: {
    watch: { actions: ["watch", "reconcileSoon"] },
    unwatch: { actions: ["unwatch", "reconcileSoon"] },
    heard: { actions: "echo" },
  },
  states: {
    closed: {
      on: { reconcile: { guard: "watched", target: "open.connecting" } },
    },
    open: {
      initial: "connecting",
      entry: "opening",
      invoke: { src: "stream", input: ({ context }) => ({ open: context.open, watching: namesOf(context.watchers) }) },
      on: {
        "stream.opened": { target: ".live", actions: ["reset", "broadcast"] },
        "stream.tick": { actions: "broadcast" },
        "stream.dropped": [{ guard: "closed", target: "waiting" }, { target: ".reconnecting" }],
        reconcile: [
          { guard: "unwatched", target: "closed" },
          { guard: "moved", target: ".reconnecting", reenter: true },
        ],
        reopen: [{ guard: "unwatched", target: "closed" }, { target: ".reconnecting", reenter: true }],
      },
      states: { connecting: {}, reconnecting: {}, live: {} },
    },
    // The stream closed by itself: it reopens after a wait that grows with each close in a row.
    waiting: {
      after: { reopen: "open.reconnecting" },
      exit: "waited",
      on: {
        reconcile: [{ guard: "unwatched", target: "closed" }, { guard: "moved", target: "open.reconnecting" }],
        reopen: [{ guard: "unwatched", target: "closed" }, { target: "open.reconnecting" }],
      },
    },
  },
});

export type SubscriptionRef = ActorRefFrom<typeof subscription>;

/** The state of the stream, as the sync chip words it (a closed stream being waited for is reconnecting). */
export function streamStatus(snapshot: SnapshotFrom<typeof subscription>): StreamStatus {
  if (snapshot.matches("closed")) {
    return "idle";
  }
  if (snapshot.matches({ open: "live" })) {
    return "live";
  }
  return snapshot.matches({ open: "connecting" }) ? "connecting" : "reconnecting";
}

/** The part of the browser's EventSource the stream uses. */
export interface EventSourceLike {
  readonly readyState: number;
  onopen: ((event: Event) => void) | null;
  onerror: ((event: Event) => void) | null;
  addEventListener(type: "tick", listener: (event: MessageEvent<string>) => void): void;
  close(): void;
}

/** EventSource's readyState once it gave up (it will not reconnect by itself). */
const EVENT_SOURCE_CLOSED = 2;

/**
 * The API's revision stream (`GET /api/events/stream?domain=...`, H6) over Server-Sent Events,
 * at `base` (the server's origin, or "" for the page's). The browser reconnects a dropped
 * stream by itself, firing `open` again; one it gave up on is reported closed.
 */
export function eventSourceTicks(
  base: string,
  create: (url: string) => EventSourceLike = (url) => new EventSource(url, { withCredentials: true }),
): OpenTicks {
  return (watching, handlers) => {
    const query = watching.map((name) => `domain=${encodeURIComponent(name)}`).join("&");
    const source = create(`${base}/api/events/stream?${query}`);
    source.onopen = () => {
      handlers.opened();
    };
    source.addEventListener("tick", (event) => {
      handlers.tick(JSON.parse(event.data) as Tick);
    });
    source.onerror = () => {
      const closed = source.readyState === EVENT_SOURCE_CLOSED;
      if (closed) {
        source.close();
      }
      handlers.dropped(closed);
    };
    return () => {
      source.close();
    };
  };
}
