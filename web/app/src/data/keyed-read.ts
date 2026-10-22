// ARCHITECTURE, Web UI: reads. One keyed-read machine runs every read the tab makes (a journey
// derived in the worker, an index, a projection, a layout, a preview), so each has the same
// answers to the same questions: is there data, is it for what is asked, is a newer read out,
// did the last one fail, and when does it try again.
//
// A key says what the read is of (what is shown), its freshness (what the answer is read from)
// and its arguments. A new `of` resets the read: what was in flight stops, what is held is
// released and forgotten. A new freshness of the same `of` keeps what is held (shown as
// refreshing, or as behind if the read for it failed) and reads again once the read in flight
// ends. A value, or a failure, shows only for its own `of`, and a failure only for its own
// freshness. One read runs at a time; ticks and key changes that arrive meanwhile coalesce into
// one more read.
//
// How the read behaves is the source's input: when it starts (at once, or after a settle for a
// preview someone is still editing), whether version skew stops it, the ticks that keep it
// current (H6), and what releases a value it holds. A transient failure is retried after the
// shared backoff (250 ms to 15 s); a refusal for authentication, the engine, version skew or a
// missing subject never is.
import {
  backoffDelayMs,
  fetched,
  forget,
  heldOf as trackedHeld,
  refetchFailed,
  reopened,
  ticked,
  UNTRACKED,
  watchName,
  type RevisionOf,
  type StreamEvent,
  type SubscriptionEvent,
  type Tick,
  type Tracked,
} from "@cairn/client";
import { HostFailure } from "@cairn/wasm";
import { assign, enqueueActions, fromCallback, fromPromise, setup, type ActorRefFrom, type AnyActorRef, type SnapshotFrom } from "xstate";

import { Missing, ReadFailed, Skewed } from "./host.ts";

/** What is shown, what it is read from, and the arguments of the read. Lists compare element by element. */
export interface ReadKey<A> {
  of: readonly unknown[];
  freshness: readonly unknown[];
  args: A;
}

/** When a read starts, and whether version skew stops it. */
export interface ReadPolicy {
  /** How long the key must hold still before the read starts: 0 reads at once. */
  settleMs: number;
  /** Whether a tab in version skew starts no new read of this kind (it derives and previews nothing). */
  stopsInSkew: boolean;
}

/** A projection of a derivation already held: answered in skew too. */
export const PROJECTION: ReadPolicy = { settleMs: 0, stopsInSkew: false };
/** A read of the host (an index, a route, a version): answered in skew too. */
export const HOST_READ: ReadPolicy = { settleMs: 0, stopsInSkew: false };
/** A read that derives from what the server sent: stopped in skew. */
export const DERIVE: ReadPolicy = { settleMs: 0, stopsInSkew: true };
/** A preview: debounced while the person edits, stopped in skew. */
export const previewPolicy = (settleMs: number): ReadPolicy => ({ settleMs, stopsInSkew: true });

/** What keeps a read current: the stream names it watches and how a tick or a value bears on it (H6). */
export interface Ticks<T> {
  /** The watch names the stream follows while the read is keyed (`journey:<id>`, `deployment`...). */
  watching: readonly string[];
  /** Whether a tick's domain is one this read shows. */
  about(of: RevisionOf): boolean;
  /** The revisions the value holds, so a tick at or below one asks for nothing. */
  holds(value: T): { of: RevisionOf; revision: number }[];
  /** The domain the read is of: its revision 0 means it no longer exists (A19). */
  subject?: RevisionOf;
  /** Whether a stream that opened again reads once more (an index lists what exists: a deletion is only an absence). */
  rereadOnReopen?: boolean;
  /** Whether a read that falls behind what the stream announced is counted by the sync chip. */
  reportsLag?: boolean;
}

/** What a read is told of the machine that runs it. */
export interface ReadAt<T> {
  /** What the machine holds from an earlier read of the same `of`. */
  held: T | undefined;
  /** Aborted when the machine no longer waits for this read's answer. */
  signal: AbortSignal;
  compatible: () => boolean;
}

export interface ReadSource<A, T> {
  read(args: A, at: ReadAt<T>): Promise<T>;
  policy: ReadPolicy;
  ticks?(args: A): Ticks<T>;
  /** Frees what a value owns, once the machine drops it: replaced, gone, a new `of`, or stopped. */
  release?(value: T): void;
}

/** A source with its types erased, as the machine holds it. */
type AnySource = ReadSource<never, unknown>;

/** That something in the tab started or ended work it owes the page: a read, or a screen's code on its way. */
export type WorkEvent = { type: "working"; id: string } | { type: "rested"; id: string };

/** Where the machine sends its watches, its shown reads and its work: the session root, which forwards the watches to the stream. */
export interface StreamPort {
  send(event: SubscriptionEvent | { type: "shown"; id: string; read: AnyActorRef } | { type: "hidden"; id: string } | WorkEvent): void;
}

/** What the session gives every read. */
export interface ReadEnv {
  stream: StreamPort;
  /** False once the tab is in version skew. */
  compatible: () => boolean;
  /** Puts the tab into version skew. */
  latch: () => void;
  /** Monotonic milliseconds, for how long a read has been behind. */
  now: () => number;
}

/** Why a read failed, in the words that decide what happens next. */
export interface ReadError {
  /** `transient`: tried again; the others are not. */
  kind: "transient" | "denied" | "refused" | "skew";
  message: string;
}

/** What a thrown failure means for the read: the subject is gone, or a kind of error. */
export function classify(thrown: unknown): ReadError | "gone" {
  if (thrown instanceof Missing) {
    return "gone";
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  if (thrown instanceof Skewed) {
    return { kind: "skew", message };
  }
  if (thrown instanceof ReadFailed) {
    return { kind: thrown.failure.status === 401 || thrown.failure.status === 403 ? "denied" : "transient", message };
  }
  if (thrown instanceof HostFailure) {
    // The engine is pure: the same input fails the same way, and a stopped module refuses everything. A derivation
    // that a newer one replaced is no refusal; reading again finds the newer document.
    return { kind: thrown.reason.error === "superseded" ? "transient" : "refused", message };
  }
  return { kind: "transient", message };
}

/**
 * Asked by a source right before the call that starts work after an awaited fetch: obsolete work
 * posts nothing. Throws an abort when the machine no longer waits for the read, and `Skewed` when the
 * tab went into version skew while the fetch was out.
 */
export function beforeWork(at: Pick<ReadAt<unknown>, "signal" | "compatible">): void {
  if (at.signal.aborted) {
    throw new DOMException("the read is no longer wanted", "AbortError");
  }
  if (!at.compatible()) {
    throw new Skewed();
  }
}

/** Whether two lists hold the same elements, compared like effect dependencies. */
export function sameKey(left: readonly unknown[] | undefined, right: readonly unknown[] | undefined): boolean {
  return left === right || (left !== undefined && right !== undefined && left.length === right.length && left.every((each, at) => Object.is(each, right[at])));
}

type Freshness = readonly unknown[];

interface Held {
  freshness: Freshness;
  value: unknown;
  /** The child that frees the value when it stops, if the source frees values. */
  owner: AnyActorRef | undefined;
}

interface Failed {
  freshness: Freshness;
  error: ReadError;
}

export interface ReadContext {
  source: AnySource;
  env: ReadEnv;
  of: readonly unknown[] | undefined;
  /** The newest key asked for; none when the caller asks for nothing now. */
  want: ReadKey<unknown> | undefined;
  ticks: Ticks<unknown> | undefined;
  held: Held | undefined;
  failure: Failed | undefined;
  /** The freshness of the read last started: what the held value is read from once it lands. */
  attempted: Freshness | undefined;
  /** A read is owed that no tick asked for: a refresh, a reopened stream, an invalidation, a retry. */
  due: boolean;
  /** Reads that failed or fell short in a row; the wait before the next one grows with it. */
  attempt: number;
  tracked: Tracked;
  /** Ticks that asked for a read not yet started, by what they name. */
  asked: Readonly<Record<string, Tick>>;
  /** Ticks the read in flight answers. */
  answering: readonly Tick[];
  /** The newest revision the stream announced of each name, apart from what was asked. */
  newest: Readonly<Record<string, number>>;
  /** When (monotonic ms) a newer revision than the one held was first announced and the value still shows the older. */
  staleSince: number | undefined;
  /** Whether the settle wait for the key wanted has run. */
  settled: boolean;
  /** Counts the children spawned, for their ids. */
  children: number;
  watcher: AnyActorRef | undefined;
  /** Whether the read is kept but not shown: it listens to nothing until shown again. */
  parked: boolean;
}

export interface ReadInput {
  source: AnySource;
  env: ReadEnv;
  parked?: boolean;
}

export type ReadEvent =
  /** The caller's key now, or none. An equal key changes nothing. */
  | { type: "want"; key: ReadKey<unknown> | undefined }
  /** Reads again: after a write this screen made, or a click on Retry. */
  | { type: "refresh" }
  /** The read stays but listens to nothing, and starts again when shown. */
  | { type: "park" }
  | { type: "show" }
  | StreamEvent;

const wantOf = (event: ReadEvent): ReadKey<unknown> | undefined => (event as Extract<ReadEvent, { type: "want" }>).key;
const tickOf = (event: ReadEvent): Tick => (event as Extract<ReadEvent, { type: "stream.tick" }>).tick;

/** Numbers the registrations with the stream: reads run in many actor systems, so an actor's own id is not unique. */
let registrations = 0;

/** What a tick does to a read. */
interface Heard {
  /** The tick names the read's subject at revision 0: it no longer exists. */
  gone: boolean;
  /** The tick calls for a read. */
  asks: boolean;
  patch: Partial<ReadContext>;
}

const NOTHING: Heard = { gone: false, asks: false, patch: {} };

/**
 * H6: what `tick` does to the read: nothing (not about it, or not newer than what it holds
 * or asked), a read to make, or the end of its subject (A19).
 */
function hear(context: ReadContext, tick: Tick, now: number): Heard {
  const ticks = context.ticks;
  if (ticks === undefined || !ticks.about(tick.of)) {
    return NOTHING;
  }
  const name = watchName(tick.of);
  if (tick.revision === 0) {
    if (ticks.subject !== undefined && watchName(ticks.subject) === name) {
      return { gone: true, asks: false, patch: {} };
    }
    // Deleted: revision 0 is older than anything held, yet the read is out of date. An invalidation, not an ask, so a
    // read in flight that still lists the domain is followed by another.
    return trackedHeld(context.tracked, tick.of) === undefined ? NOTHING : { gone: false, asks: true, patch: { tracked: forget(context.tracked, tick.of), due: true } };
  }
  const next = ticked(context.tracked, tick);
  if (next === undefined) {
    return NOTHING;
  }
  return {
    gone: false,
    asks: true,
    patch: {
      tracked: next,
      asked: { ...context.asked, [name]: tick },
      newest: { ...context.newest, [name]: Math.max(context.newest[name] ?? 0, tick.revision) },
      staleSince: context.held === undefined ? context.staleSince : (context.staleSince ?? now),
    },
  };
}

/** What a read that answered leaves: the tracking, the lag, and whether to read again. */
interface Landing {
  patch: Partial<ReadContext>;
  /** The answer holds a domain below a revision announced before the read began. */
  undershot: boolean;
}

/**
 * The answer `value` lands. What it holds is credited first; a tick asked before the read began is
 * credited at its own revision only for a domain the answer does not hold (an index answers for a
 * journey outside its filter by not listing it), and forgotten for a held domain the answer did not
 * reach, so its revision asks again instead of being credited a revision never fetched. Ticks asked
 * during the read stay while still newer than what is held.
 */
function landing(context: ReadContext, value: unknown): Landing {
  const holds = context.ticks?.holds(value) ?? [];
  const names = new Set(holds.map(({ of }) => watchName(of)));
  let tracked = holds.reduce((each, { of, revision }) => fetched(each, of, revision), context.tracked);
  let undershot = false;
  for (const tick of context.answering) {
    if (!names.has(watchName(tick.of))) {
      tracked = fetched(tracked, tick.of, tick.revision);
    } else if ((trackedHeld(tracked, tick.of) ?? 0) < tick.revision) {
      tracked = refetchFailed(tracked, tick.of);
      undershot = true;
    }
  }
  const asked = Object.fromEntries(Object.entries(context.asked).filter(([, tick]) => (trackedHeld(tracked, tick.of) ?? -1) < tick.revision));
  const behind = Object.entries(context.newest).some(([name, revision]) => (tracked.held[name] ?? 0) < revision);
  return { patch: { tracked, asked, answering: [], staleSince: behind ? context.staleSince : undefined }, undershot };
}

/** The freshness a read for the key wanted starts from. */
const freshOf = (context: ReadContext): Freshness | undefined => context.want?.freshness;

const pending = (context: ReadContext): boolean =>
  context.due || Object.keys(context.asked).length > 0 || !sameKey(context.attempted, freshOf(context));

/** What the read answered, and what it threw: the events XState sends the machine for its invoked read. */
const outputOf = (event: unknown): unknown => (event as { output: unknown }).output;
const thrownOf = (event: unknown): unknown => (event as { error: unknown }).error;

const fresh = (context: ReadContext): Partial<ReadContext> => ({
  held: undefined,
  failure: undefined,
  attempted: undefined,
  due: false,
  attempt: 0,
  tracked: UNTRACKED,
  asked: {},
  answering: [],
  newest: {},
  staleSince: undefined,
  settled: false,
  ticks: undefined,
  watcher: undefined,
  children: context.children,
});

export const keyedRead = setup({
  types: { context: {} as ReadContext, events: {} as ReadEvent, input: {} as ReadInput },
  actors: {
    // A source that throws before it returns a promise (the check before work) fails the read like one that rejects.
    read: fromPromise<unknown, { source: AnySource; args: unknown; held: unknown; compatible: () => boolean }>(
      async ({ input, signal }) => await input.source.read(input.args as never, { held: input.held, signal, compatible: input.compatible }),
    ),
    // Registers the read with the stream while it is keyed, and leaves it when stopped.
    watcher: fromCallback<StreamEvent, { stream: StreamPort; names: readonly string[]; read: AnyActorRef }>(({ input }) => {
      registrations += 1;
      const id = `watch-${String(registrations)}`;
      input.stream.send({ type: "watch", id, names: input.names, to: input.read });
      return () => {
        input.stream.send({ type: "unwatch", id });
      };
    }),
    // Tells the session a shown read is keyed, so the sync chip counts it, and leaves when stopped.
    reporter: fromCallback<StreamEvent, { stream: StreamPort; read: AnyActorRef; reports: boolean }>(({ input }) => {
      if (!input.reports) {
        return;
      }
      registrations += 1;
      const id = `shown-${String(registrations)}`;
      input.stream.send({ type: "shown", id, read: input.read });
      return () => {
        input.stream.send({ type: "hidden", id });
      };
    }),
    // Tells the session the read is working (settling or reading) for as long as it runs. Waiting out a
    // failure is not work: the page shows the failure until the clock starts the next read.
    work: fromCallback<StreamEvent, { stream: StreamPort }>(({ input }) => {
      registrations += 1;
      const id = `work-${String(registrations)}`;
      input.stream.send({ type: "working", id });
      return () => {
        input.stream.send({ type: "rested", id });
      };
    }),
    // Owns one held value; stopping it frees the value.
    owner: fromCallback<StreamEvent, { source: AnySource; value: unknown }>(({ input }) => () => {
      input.source.release?.(input.value);
    }),
  },
  delays: {
    settle: ({ context }) => context.source.policy.settleMs,
    // `attempt` counts the failures including the one waited out.
    retry: ({ context }) => backoffDelayMs(context.attempt - 1),
  },
  guards: {
    keyed: ({ event }) => wantOf(event) !== undefined,
    newOf: ({ context, event }) => {
      const key = wantOf(event);
      return key !== undefined && !sameKey(context.of, key.of);
    },
    unwanted: ({ context, event }) => wantOf(event) === undefined && context.want !== undefined,
    newFreshness: ({ context, event }) => {
      const key = wantOf(event);
      return key !== undefined && sameKey(context.of, key.of) && !sameKey(freshOf(context), key.freshness);
    },
    nothingWanted: ({ context }) => context.want === undefined,
    quiet: ({ context }) => !pending(context),
    skewed: ({ context }) => !context.env.compatible() && (context.source.policy.stopsInSkew || context.attempt > 0),
    unsettled: ({ context }) => context.source.policy.settleMs > 0 && !context.settled,
    subjectGone: ({ context, event }) => hear(context, tickOf(event), 0).gone,
    asks: ({ context, event }) => hear(context, tickOf(event), 0).asks,
    rereads: ({ context }) => context.ticks?.rereadOnReopen === true && context.held !== undefined,
    isGone: ({ event }) => classify(thrownOf(event)) === "gone",
    isSkew: ({ event }) => {
      const error = classify(thrownOf(event));
      return error !== "gone" && error.kind === "skew";
    },
    obsolete: ({ context }) => !sameKey(context.attempted, freshOf(context)),
    isTransient: ({ event }) => {
      const error = classify(thrownOf(event));
      return error !== "gone" && error.kind === "transient";
    },
    undershot: ({ context, event }) => landing(context, outputOf(event)).undershot,
    parked: ({ context }) => context.parked,
    // A domain deleted that the read holds nothing of yet: a read in flight may still list it.
    deletedUnheld: ({ context, event }) => {
      const tick = tickOf(event);
      const ticks = context.ticks;
      return tick.revision === 0 && ticks !== undefined && ticks.about(tick.of) && !(ticks.subject !== undefined && watchName(ticks.subject) === watchName(tick.of)) && trackedHeld(context.tracked, tick.of) === undefined;
    },
  },
  actions: {
    begin: assign(({ context, event }) => {
      const key = wantOf(event);
      return { ...fresh(context), of: key?.of, want: key, ticks: key === undefined ? undefined : context.source.ticks?.(key.args as never) };
    }),
    wantMore: assign(({ event }) => ({ want: wantOf(event), settled: false })),
    unwant: assign({ want: undefined }),
    // The read in flight is cancelled, not answered: nothing was attempted, and the ticks it answered are asked again.
    cancelled: assign(({ context }) => ({
      attempted: context.held?.freshness,
      asked: Object.fromEntries([...context.answering.map((tick) => [watchName(tick.of), tick] as const), ...Object.entries(context.asked)]),
      answering: [],
    })),
    // Freeing what is held: the owner child frees the value as it stops.
    releaseHeld: enqueueActions(({ context, enqueue }) => {
      if (context.held?.owner !== undefined) {
        enqueue.stopChild(context.held.owner);
      }
      enqueue.assign({ held: undefined });
    }),
    releaseReplaced: enqueueActions(({ context, event, enqueue }) => {
      if (context.held?.owner !== undefined && context.held.value !== outputOf(event)) {
        enqueue.stopChild(context.held.owner);
      }
    }),
    startWatching: assign(({ context, self, spawn }) => {
      if (context.parked || context.watcher !== undefined || context.ticks === undefined || context.ticks.watching.length === 0) {
        return {};
      }
      const id = `watch-${String(context.children)}`;
      return { children: context.children + 1, watcher: spawn("watcher", { id, input: { stream: context.env.stream, names: context.ticks.watching, read: self } }) };
    }),
    stopWatching: enqueueActions(({ context, enqueue }) => {
      if (context.watcher !== undefined) {
        enqueue.stopChild(context.watcher);
        enqueue.assign({ watcher: undefined });
      }
    }),
    parked: assign({ parked: true }),
    shown: assign({ parked: false }),
    hear: assign(({ context, event }) => hear(context, tickOf(event), context.env.now()).patch),
    // The stream opened again: whatever was asked and not read is forgotten, and the current revisions ask again.
    reopened: assign(({ context }) => ({ tracked: reopened(context.tracked), due: context.due || (context.ticks?.rereadOnReopen === true && context.held !== undefined) })),
    owe: assign({ due: true }),
    ripe: assign({ due: true, settled: true }),
    settle: assign({ settled: true }),
    starting: assign(({ context }) => ({ answering: Object.values(context.asked), asked: {}, due: false, attempted: freshOf(context) })),
    land: assign(({ context, event, spawn }) => {
      const value = outputOf(event);
      const landed = landing(context, value);
      const previous = context.held;
      let owner = previous?.owner;
      let children = context.children;
      if (context.source.release !== undefined && previous?.value !== value) {
        owner = spawn("owner", { id: `owner-${String(children)}`, input: { source: context.source, value } });
        children += 1;
      }
      return {
        ...landed.patch,
        held: { freshness: context.attempted ?? [], value, owner },
        failure: undefined,
        attempt: landed.undershot ? context.attempt + 1 : 0,
        children,
      };
    }),
    // The read failed: what it answered is asked again after the failure, and the next tick asks again.
    failed: assign(({ context, event }) => {
      const error = classify(thrownOf(event));
      return {
        failure: error === "gone" ? context.failure : { freshness: context.attempted ?? [], error },
        asked: Object.fromEntries([...context.answering.map((tick) => [watchName(tick.of), tick] as const), ...Object.entries(context.asked)]),
        answering: [],
        tracked: reopened(context.tracked),
        attempt: context.attempt + 1,
      };
    }),
    // A failure for a freshness since replaced is only forgotten: the newer one is read now.
    forgetAnswering: assign(({ context }) => ({
      asked: Object.fromEntries([...context.answering.map((tick) => [watchName(tick.of), tick] as const), ...Object.entries(context.asked)]),
      answering: [],
    })),
    latch: ({ context }) => {
      context.env.latch();
    },
    // The read ended with nothing: its subject is gone.
    ended: assign(({ context }) => ({
      failure: undefined,
      asked: {},
      answering: [],
      newest: {},
      staleSince: undefined,
      attempt: 0,
      due: false,
      tracked: context.ticks?.subject === undefined ? context.tracked : forget(context.tracked, context.ticks.subject),
    })),
    stopped: assign(({ context }) => ({
      failure: context.failure !== undefined && sameKey(context.failure.freshness, freshOf(context)) ? context.failure : { freshness: freshOf(context) ?? [], error: { kind: "skew" as const, message: "the server runs another version of Cairn" } },
    })),
  },
}).createMachine({
  id: "keyedRead",
  context: ({ input }) => ({
    source: input.source,
    env: input.env,
    of: undefined,
    want: undefined,
    ticks: undefined,
    held: undefined,
    failure: undefined,
    attempted: undefined,
    due: false,
    attempt: 0,
    tracked: UNTRACKED,
    asked: {},
    answering: [],
    newest: {},
    staleSince: undefined,
    settled: false,
    children: 0,
    watcher: undefined,
    parked: input.parked ?? false,
  }),
  initial: "idle",
  states: {
    idle: { on: { want: { guard: "keyed", target: "keyed", actions: "begin" } } },
    keyed: {
      initial: "deciding",
      entry: "startWatching",
      exit: "stopWatching",
      invoke: {
        id: "reporter",
        src: "reporter",
        input: ({ context, self }) => ({ stream: context.env.stream, read: self, reports: context.ticks?.reportsLag === true }),
      },
      on: {
        want: [
          { guard: "newOf", target: "keyed", reenter: true, actions: ["releaseHeld", "begin"] },
          { guard: "unwanted", target: ".deciding", actions: "unwant" },
          { guard: "newFreshness", actions: "wantMore" },
        ],
        "stream.tick": [
          { guard: "subjectGone", target: ".gone" },
          { guard: "asks", actions: "hear" },
        ],
        "stream.opened": { actions: "reopened" },
        refresh: { actions: "owe" },
        park: { actions: ["parked", "stopWatching"] },
        show: { actions: ["shown", "startWatching"] },
      },
      states: {
        // Where every path into a read passes, the settle timer and the retry timer included, so
        // version skew is asked at each of them.
        deciding: {
          always: [
            { guard: "nothingWanted", target: "resting" },
            { guard: "quiet", target: "resting" },
            { guard: "skewed", target: "stopped" },
            { guard: "unsettled", target: "settling" },
            { target: "reading" },
          ],
        },
        settling: {
          tags: "busy",
          invoke: { src: "work", input: ({ context }) => ({ stream: context.env.stream }) },
          after: { settle: { target: "deciding", actions: "settle" } },
          on: { want: { guard: "newFreshness", target: "settling", reenter: true, actions: "wantMore" } },
        },
        reading: {
          tags: "busy",
          entry: "starting",
          on: {
            want: { guard: "unwanted", target: "deciding", actions: ["unwant", "cancelled"] },
            // The deletion of a domain this read does not hold may be missed by the answer on its way.
            "stream.tick": { guard: "deletedUnheld", actions: "owe" },
          },
          invoke: [
            { src: "work", input: ({ context }) => ({ stream: context.env.stream }) },
            {
              id: "read",
              src: "read",
              input: ({ context }) => ({ source: context.source, args: context.want?.args, held: context.held?.value, compatible: context.env.compatible }),
              onDone: [
                { guard: "undershot", target: "backoff", actions: ["releaseReplaced", "land"] },
                { target: "deciding", actions: ["releaseReplaced", "land"] },
              ],
              onError: [
                { guard: "isGone", target: "gone" },
                { guard: "isSkew", target: "stopped", actions: ["failed", "latch"] },
                { guard: "obsolete", target: "deciding", actions: "forgetAnswering" },
                { guard: "isTransient", target: "backoff", actions: "failed" },
                { target: "resting", actions: "failed" },
              ],
            },
          ],
        },
        // Waiting out a failed read, or an answer short of what was announced.
        backoff: {
          after: { retry: { target: "deciding", actions: "owe" } },
          on: {
            "stream.tick": [{ guard: "subjectGone", target: "gone" }, { guard: "asks", target: "deciding", actions: "hear" }],
            "stream.opened": { guard: "rereads", target: "deciding", actions: "reopened" },
            refresh: { target: "deciding", actions: "owe" },
            want: { guard: "newFreshness", target: "deciding", actions: "wantMore" },
          },
        },
        resting: {
          on: {
            "stream.tick": [{ guard: "subjectGone", target: "gone" }, { guard: "asks", target: "deciding", actions: "hear" }],
            "stream.opened": { guard: "rereads", target: "deciding", actions: "reopened" },
            refresh: { target: "deciding", actions: "owe" },
            want: { guard: "newFreshness", target: "deciding", actions: "wantMore" },
          },
        },
        // Version skew stops new work; it never lifts.
        stopped: { entry: "stopped", on: { "stream.tick": {}, "stream.opened": {}, refresh: {} } },
        // The subject no longer exists (A19): left only for another `of`, or by a refresh.
        gone: {
          entry: ["releaseHeld", "ended"],
          on: { "stream.tick": {}, "stream.opened": {}, refresh: { target: "deciding", actions: "owe" } },
        },
      },
    },
  },
});

export type ReadRef = ActorRefFrom<typeof keyedRead>;
export type ReadSnapshot = SnapshotFrom<typeof keyedRead>;

/** What a read shows its caller. */
export type ReadView<T> =
  | { status: "idle" }
  | { status: "loading" }
  /** The first read failed: nothing to show. `retrying`: a transient failure is tried again. */
  | { status: "failed"; error: ReadError; retrying: boolean }
  | { status: "current"; value: T }
  /** What is held is read from an older freshness; a read for the current one is out or owed. */
  | { status: "refreshing"; value: T }
  /** A read for the current freshness failed: the held value stays. */
  | { status: "behind"; value: T; error: ReadError; retrying: boolean }
  | { status: "gone" };

const IDLE = { status: "idle" } as const;
const LOADING = { status: "loading" } as const;
const GONE = { status: "gone" } as const;

/**
 * What the read shows for `shown` (what it is of) read from `freshness`. Another `of`'s value
 * or failure never shows (a caller whose key moved sees the read catch up on its next turn), and
 * a failure shows only for the freshness it failed at. Without a freshness a value held is current.
 */
export function readView<T>(snapshot: ReadSnapshot, shown: readonly unknown[] | undefined, freshness: readonly unknown[] | undefined): ReadView<T> {
  const { context } = snapshot;
  if (shown === undefined) {
    return IDLE;
  }
  if (!sameKey(context.of, shown)) {
    return LOADING;
  }
  if (snapshot.matches({ keyed: "gone" })) {
    return GONE;
  }
  const failure = context.failure !== undefined && freshness !== undefined && sameKey(context.failure.freshness, freshness) ? context.failure : undefined;
  const retrying = failure?.error.kind === "transient" && !snapshot.matches({ keyed: "stopped" });
  const held = context.held;
  if (held !== undefined) {
    const value = held.value as T;
    if (failure !== undefined) {
      return { status: "behind", value, error: failure.error, retrying };
    }
    return freshness === undefined || sameKey(held.freshness, freshness) ? { status: "current", value } : { status: "refreshing", value };
  }
  return failure === undefined ? LOADING : { status: "failed", error: failure.error, retrying };
}

/** The value a view shows, if it shows one. */
export function valueOf<T>(view: ReadView<T>): T | undefined {
  return view.status === "current" || view.status === "refreshing" || view.status === "behind" ? view.value : undefined;
}

/** The words of the failure a view shows in place of a value, if it does. */
export function errorWords<T>(view: ReadView<T>): string | undefined {
  return view.status === "failed" ? view.error.message : undefined;
}

/** The value the read holds, whatever it is read from (the journey manager reads this). */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the reader knows the source its read runs
export function heldValue<T>(snapshot: ReadSnapshot): T | undefined {
  return snapshot.context.held?.value as T | undefined;
}

/** Whether the read has answered or failed at least once and is not working now. */
export function settledRead(snapshot: ReadSnapshot): boolean {
  return snapshot.context.attempted !== undefined && !snapshot.hasTag("busy");
}

/** A shown read that is behind the revision the stream announced (the sync chip's UPDATING and BEHIND). */
export interface Lag {
  /** The watch name of the subject. */
  subject: string;
  shown: number;
  announced: number;
  since: number;
  failed: boolean;
}

/** How far a read that holds a value is behind its subject, if it is. */
export function readLag(snapshot: ReadSnapshot): Lag | undefined {
  const { ticks, staleSince, held, newest, tracked, failure } = snapshot.context;
  if (ticks?.subject === undefined || staleSince === undefined || held === undefined) {
    return undefined;
  }
  const subject = watchName(ticks.subject);
  const shown = tracked.held[subject] ?? 0;
  return { subject, shown, announced: Math.max(newest[subject] ?? shown, shown), since: staleSince, failed: failure !== undefined };
}

/** The lag the chip shows: a failed refetch outranks one still out; among alike, the one that has waited longest. */
export function worstLag(lags: readonly (Lag | undefined)[]): Lag | undefined {
  let worst: Lag | undefined;
  for (const lag of lags) {
    if (lag !== undefined && (worst === undefined || (lag.failed && !worst.failed) || (lag.failed === worst.failed && lag.since < worst.since))) {
      worst = lag;
    }
  }
  return worst;
}
