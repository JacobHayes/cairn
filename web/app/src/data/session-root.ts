// ARCHITECTURE, Web UI: the session root. Started once at boot, outside React and StrictMode,
// it holds what the whole tab shares and its children: whether the tab is compatible with the
// server's build or in version skew (which absorbs the old latch), the saves the tab made
// (plain data, newest first), the tick stream (the subscription), the deployment and the
// caller (keyed reads), the journeys (the journey manager), the edit manager, which owns every
// write, the sync scheduler (the facts that carry a deadline, and a wake at the next), the
// toast, and the conversation manager (one conversation per assistant target). The root
// forwards what reads send the stream, tells the scheduler what only it sees (a save landed,
// the count of writes in flight), keeps the shown reads (the ones the sync chip counts
// when they fall behind) and the work under way (each read working and each screen whose code is
// on its way, which the chip's settled fact reads), and tells the stream each commit a write of
// this tab made, so the reads fetch it without waiting for the server's tick.
//
// Skew never lifts, so `skewed` is not final: a final root would stop its children, and the
// kept writes with them. In skew the tab derives, previews, writes and retries nothing (the
// old engine's overlap check may be wrong) and asks for a reload; unsent edits are records in
// session storage, so the reload keeps them. Stopping the root stops every child: the stream
// closes, the timers cancel, and the derivations held are released.
import { streamStatus, subscription, type OpenTicks, type StreamStatus, type SubscriptionEvent, type Tick } from "@cairn/client";
import { assign, enqueueActions, sendTo, setup, type ActorRefFrom, type AnyActorRef } from "xstate";

import { conversationManager, type ConversationsInput, type ConversationsRef } from "../assistant/conversations.ts";
import { NO_SAVES, recorded, type Recent, type SaveEvent, type TitleOf } from "./activity.ts";
import { editManager, inFlightOf, type EditsInput, type EditsRef } from "./edit-manager.ts";
import type { Deriver, Host } from "./host.ts";
import { journeyManager } from "./journeys.ts";
import { keyedRead, type ReadEnv, type WorkEvent } from "./keyed-read.ts";
import { DEPLOYMENT_KEY, deploymentSource, callerKey, callerSource } from "./session-reads.ts";
import { syncMachine, type SyncRef } from "./sync.ts";
import { toastMachine, type ToastsRef } from "./toasts.ts";
import type { Answer, Command } from "./edit-record.ts";
import type { WriteEnv } from "./write-machine.ts";
import { committedTick, saveEventOf, saveOf } from "./writes.ts";

/** What the root is started with: the host, and the manager's input less what the root itself answers. */
export interface RootInput {
  host: Host;
  deriver: Deriver;
  open: OpenTicks;
  /** The page's clock, for date rollover. */
  today: () => Date;
  /** Monotonic milliseconds: the clock reads measure their lag on, and the sync scheduler counts its thresholds on. */
  now: () => number;
  conversations: ConversationsInput;
  edits: Omit<EditsInput, "env"> & { env: Omit<WriteEnv, "compatible"> };
}

export interface RootContext extends RootInput {
  /** The saves this tab made, newest first. */
  recent: Recent;
  /** The reads the sync chip counts when they fall behind, by registration. */
  shown: Readonly<Record<string, AnyActorRef>>;
  /** The stream's state as last seen, so a change wakes the waiting writes. */
  stream: StreamStatus;
  /** The work under way, by registration: reads settling or reading, and screens whose code is loading. */
  working: Readonly<Record<string, true>>;
}

export type RootEvent =
  | { type: "skew" }
  /** The connection changed: whatever waits looks again. */
  | { type: "wake" }
  /** Puts the kept edits back, once the root is running. */
  | { type: "restore" }
  /** A write landed and left this save. */
  | { type: "saved"; save: Omit<SaveEvent, "id"> }
  /** A write of this tab committed: its receipt is a tick the reads hear before the stream's. */
  | { type: "committed"; tick: Tick }
  | WorkEvent
  | SubscriptionEvent
  /** A shown read is keyed; `hidden` when it stops or changes its `of`. */
  | { type: "shown"; id: string; read: AnyActorRef }
  | { type: "hidden"; id: string }
  | { type: "xstate.snapshot.subscription"; snapshot: SubscriptionSnapshot }
  | { type: "xstate.snapshot.deployment"; snapshot: DeploymentSnapshot }
  | { type: "xstate.snapshot.edits"; snapshot: ReturnType<EditsRef["getSnapshot"]> };

type SubscriptionSnapshot = ReturnType<ActorRefFrom<typeof subscription>["getSnapshot"]>;
type DeploymentSnapshot = ReturnType<ActorRefFrom<typeof keyedRead>["getSnapshot"]>;

const without = <T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> => Object.fromEntries(Object.entries(record).filter(([each]) => each !== key));

export const sessionRoot = setup({
  types: { context: {} as RootContext, events: {} as RootEvent, input: {} as RootInput },
  actors: { subscription, read: keyedRead, journeys: journeyManager, edits: editManager, sync: syncMachine, toasts: toastMachine, conversations: conversationManager },
  actions: {
    // The reads and the manager send the stream through the root, which forwards to the subscription.
    forward: sendTo("subscription", ({ event }) => event as SubscriptionEvent),
    showRead: assign(({ context, event }) => {
      const shown = event as Extract<RootEvent, { type: "shown" }>;
      return { shown: { ...context.shown, [shown.id]: shown.read } };
    }),
    hideRead: assign(({ context, event }) => ({ shown: without(context.shown, (event as Extract<RootEvent, { type: "hidden" }>).id) })),
    work: assign(({ context, event }) => ({ working: { ...context.working, [(event as Extract<RootEvent, { type: "working" }>).id]: true as const } })),
    rest: assign(({ context, event }) => ({ working: without(context.working, (event as Extract<RootEvent, { type: "rested" }>).id) })),
    // The stream's watchers hear the commit as they would the server's tick for it, which then asks for nothing.
    hear: sendTo("subscription", ({ event }) => ({ type: "heard" as const, tick: (event as Extract<RootEvent, { type: "committed" }>).tick })),
    // A save landed: the tab keeps it.
    keepSave: assign({ recent: ({ context, event }) => recorded(context.recent, (event as Extract<RootEvent, { type: "saved" }>).save) }),
    // The edit manager moved: the scheduler hears how many writes are in flight (and ignores a repeat).
    countInFlight: sendTo("sync", ({ event }) => ({ type: "inFlight" as const, count: inFlightOf((event as Extract<RootEvent, { type: "xstate.snapshot.edits" }>).snapshot.context.writes) })),
    wantDeployment: sendTo("deployment", { type: "want", key: DEPLOYMENT_KEY }),
    // The deployment moved: the caller is read once its first read settled, and again for each newer one held.
    keyCaller: sendTo("caller", ({ event }) => ({ type: "want" as const, key: callerKey((event as Extract<RootEvent, { type: "xstate.snapshot.deployment" }>).snapshot) })),
    // A change of the stream's state wakes the writes waiting on the connection.
    streamMoved: enqueueActions(({ context, event, enqueue }) => {
      const status = streamStatus((event as Extract<RootEvent, { type: "xstate.snapshot.subscription" }>).snapshot);
      if (status !== context.stream) {
        enqueue.assign({ stream: status });
        enqueue.sendTo("edits", { type: "wake" });
      }
    }),
  },
}).createMachine({
  id: "session",
  context: ({ input }) => ({ ...input, recent: NO_SAVES, shown: {}, stream: "idle", working: {} }),
  // The scheduler and the toast are first: the others' relays and notes reach them.
  invoke: [
    { id: "sync", src: "sync", input: ({ context }) => ({ monotonic: context.now }) },
    { id: "toasts", src: "toasts" },
    { id: "subscription", src: "subscription", input: ({ context }) => ({ open: context.open }), onSnapshot: { actions: "streamMoved" } },
    {
      id: "deployment",
      src: "read",
      input: ({ context, self }) => ({ source: deploymentSource(context.host), env: readEnvOf(self, context.now) }),
      onSnapshot: { actions: "keyCaller" },
    },
    { id: "caller", src: "read", input: ({ context, self }) => ({ source: callerSource(context.host), env: readEnvOf(self, context.now) }) },
    {
      id: "journeys",
      src: "journeys",
      input: ({ context, self }) => ({ host: context.host, deriver: context.deriver, env: readEnvOf(self, context.now), today: context.today }),
    },
    {
      id: "edits",
      src: "edits",
      input: ({ context, self }) => ({
        ...context.edits,
        env: { ...context.edits.env, compatible: () => self.getSnapshot().matches("compatible") },
      }),
      onSnapshot: { actions: "countInFlight" },
    },
    { id: "conversations", src: "conversations", input: ({ context }) => context.conversations },
  ],
  initial: "starting",
  states: {
    // The children exist once the root's first step is over: the deployment is asked for then.
    starting: { always: { target: "compatible", actions: "wantDeployment" } },
    compatible: { on: { skew: "skewed" } },
    // Waiting writes give up at once: the wake is delivered after this transition commits.
    skewed: { entry: sendTo("edits", { type: "wake" }) },
  },
  on: {
    wake: { actions: sendTo("edits", { type: "wake" }) },
    restore: { actions: sendTo("edits", { type: "restore" }) },
    // The scheduler counts the save's SAVED time from now.
    saved: { actions: ["keepSave", sendTo("sync", { type: "saved" })] },
    watch: { actions: "forward" },
    unwatch: { actions: "forward" },
    reopen: { actions: "forward" },
    shown: { actions: "showRead" },
    hidden: { actions: "hideRead" },
    working: { actions: "work" },
    rested: { actions: "rest" },
    committed: { actions: "hear" },
  },
});

export type SessionRootRef = ActorRefFrom<typeof sessionRoot>;
export type { EditsRef };
export type JourneysRef = ActorRefFrom<typeof journeyManager>;
export type SubscriptionRef = ActorRefFrom<typeof subscription>;
export type { ConversationsRef, SyncRef, ToastsRef };
export type ReadRef = ActorRefFrom<typeof keyedRead>;

/**
 * What the root is told of a write that landed at `at`: the commit it made, which the reads fetch
 * now rather than when the server's coalesced tick for it arrives, and the save it leaves.
 */
export function landedEvents(command: Command, answer: Answer, began: number, titleOf: TitleOf, at: Date): RootEvent[] {
  const tick = committedTick(answer);
  const save = saveOf(command, answer, began, titleOf);
  return [
    ...(tick === undefined ? [] : [{ type: "committed" as const, tick }]),
    ...(save === undefined ? [] : [{ type: "saved" as const, save: saveEventOf(save, titleOf, at) }]),
  ];
}

/** The part of the root a read's environment uses. */
interface RootPort {
  send(event: RootEvent): void;
  getSnapshot(): { matches(state: "compatible"): boolean };
}

/**
 * What the reads are given of the session: the stream through the root, whether the tab is
 * compatible, the way into skew, and the clock. Closures over the root, which is still being
 * constructed when its children are, so nothing here asks for its snapshot until a read runs.
 */
export function readEnvOf(root: RootPort, now: () => number): ReadEnv {
  return {
    stream: root,
    compatible: () => root.getSnapshot().matches("compatible"),
    latch: () => {
      root.send({ type: "skew" });
    },
    now,
  };
}

function childOf<K extends "edits" | "journeys" | "subscription" | "deployment" | "caller" | "sync" | "toasts" | "conversations">(root: SessionRootRef, id: K) {
  const child = root.getSnapshot().children[id];
  if (child === undefined) {
    throw new Error(`the session root is not running its ${id}`);
  }
  return child;
}

/** The root's edit manager. */
export const editsOf = (root: SessionRootRef): EditsRef => childOf(root, "edits");
/** The root's journey manager. */
export const journeysOf = (root: SessionRootRef): JourneysRef => childOf(root, "journeys");
/** The root's tick stream. */
export const subscriptionOf = (root: SessionRootRef): SubscriptionRef => childOf(root, "subscription");
/** The root's deployment read. */
export const deploymentOf = (root: SessionRootRef): ReadRef => childOf(root, "deployment");
/** The root's caller read. */
export const callerOf = (root: SessionRootRef): ReadRef => childOf(root, "caller");
/** The root's sync scheduler. */
export const syncOf = (root: SessionRootRef): SyncRef => childOf(root, "sync");
/** The root's toast. */
export const toastsOf = (root: SessionRootRef): ToastsRef => childOf(root, "toasts");
/** The root's conversation manager. */
export const conversationsOf = (root: SessionRootRef): ConversationsRef => childOf(root, "conversations");
