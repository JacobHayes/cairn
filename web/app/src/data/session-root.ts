// ARCHITECTURE, Web UI: the session root. Started once at boot, outside React and StrictMode,
// it holds what the whole tab shares and its children: whether the tab is compatible with the
// server's build or in version skew (which absorbs the old latch), the tick stream (the
// subscription), the deployment and the caller (keyed reads), the journeys (the journey manager),
// and the edit manager, which owns every write. The root forwards what reads send the stream, and
// keeps the shown reads (the ones the sync chip counts when they fall behind). The other owners
// (the activity, the sync status) join as their steps land.
//
// Skew never lifts, so `skewed` is not final: a final root would stop its children, and the
// kept writes with them. In skew the tab derives, previews, writes and retries nothing (the
// old engine's touched sets may be wrong) and asks for a reload; unsent edits are records in
// session storage, so the reload keeps them. Stopping the root stops every child: the stream
// closes, the timers cancel, and the derivations held are released.
import { streamStatus, subscription, type OpenTicks, type StreamStatus, type SubscriptionEvent } from "@cairn/client";
import { assign, enqueueActions, sendTo, setup, type ActorRefFrom, type AnyActorRef } from "xstate";

import { editManager, type EditsInput, type EditsRef } from "./edit-manager.ts";
import type { Deriver, Host } from "./host.ts";
import { journeyManager } from "./journeys.ts";
import { keyedRead, type ReadEnv } from "./keyed-read.ts";
import { DEPLOYMENT_KEY, deploymentSource, viewerKey, viewerSource } from "./session-reads.ts";
import type { WriteEnv } from "./write-machine.ts";

/** What the root is started with: the host, and the manager's input less what the root itself answers. */
export interface RootInput {
  host: Host;
  deriver: Deriver;
  open: OpenTicks;
  /** The page's clock, for date rollover. */
  today: () => Date;
  /** Monotonic milliseconds. */
  now: () => number;
  edits: Omit<EditsInput, "env"> & { env: Omit<WriteEnv, "compatible"> };
}

export interface RootContext extends RootInput {
  /** The reads the sync chip counts when they fall behind, by registration. */
  shown: Readonly<Record<string, AnyActorRef>>;
  /** The stream's state as last seen, so a change wakes the waiting writes. */
  stream: StreamStatus;
}

export type RootEvent =
  | { type: "skew" }
  /** The connection changed: whatever waits looks again. */
  | { type: "wake" }
  /** Puts the kept edits back, once the root is running. */
  | { type: "restore" }
  | SubscriptionEvent
  /** A shown read is keyed; `hidden` when it stops or changes identity. */
  | { type: "shown"; id: string; read: AnyActorRef }
  | { type: "hidden"; id: string }
  | { type: "xstate.snapshot.subscription"; snapshot: SubscriptionSnapshot }
  | { type: "xstate.snapshot.deployment"; snapshot: DeploymentSnapshot };

type SubscriptionSnapshot = ReturnType<ActorRefFrom<typeof subscription>["getSnapshot"]>;
type DeploymentSnapshot = ReturnType<ActorRefFrom<typeof keyedRead>["getSnapshot"]>;

const without = <T>(record: Readonly<Record<string, T>>, key: string): Record<string, T> => Object.fromEntries(Object.entries(record).filter(([each]) => each !== key));

export const sessionRoot = setup({
  types: { context: {} as RootContext, events: {} as RootEvent, input: {} as RootInput },
  actors: { subscription, read: keyedRead, journeys: journeyManager, edits: editManager },
  actions: {
    // The reads and the manager send the stream through the root, which forwards to the subscription.
    forward: sendTo("subscription", ({ event }) => event as SubscriptionEvent),
    showRead: assign(({ context, event }) => {
      const shown = event as Extract<RootEvent, { type: "shown" }>;
      return { shown: { ...context.shown, [shown.id]: shown.read } };
    }),
    hideRead: assign(({ context, event }) => ({ shown: without(context.shown, (event as Extract<RootEvent, { type: "hidden" }>).id) })),
    wantDeployment: sendTo("deployment", { type: "want", key: DEPLOYMENT_KEY }),
    // The deployment moved: the caller is read once its first read settled, and again for each newer one held.
    keyViewer: sendTo("viewer", ({ event }) => ({ type: "want" as const, key: viewerKey((event as Extract<RootEvent, { type: "xstate.snapshot.deployment" }>).snapshot) })),
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
  context: ({ input }) => ({ ...input, shown: {}, stream: "idle" }),
  invoke: [
    { id: "subscription", src: "subscription", input: ({ context }) => ({ open: context.open }), onSnapshot: { actions: "streamMoved" } },
    {
      id: "deployment",
      src: "read",
      input: ({ context, self }) => ({ source: deploymentSource(context.host), env: readEnvOf(self, context.now) }),
      onSnapshot: { actions: "keyViewer" },
    },
    { id: "viewer", src: "read", input: ({ context, self }) => ({ source: viewerSource(context.host), env: readEnvOf(self, context.now) }) },
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
    },
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
    watch: { actions: "forward" },
    unwatch: { actions: "forward" },
    reopen: { actions: "forward" },
    shown: { actions: "showRead" },
    hidden: { actions: "hideRead" },
  },
});

export type SessionRootRef = ActorRefFrom<typeof sessionRoot>;
export type { EditsRef };
export type JourneysRef = ActorRefFrom<typeof journeyManager>;
export type SubscriptionRef = ActorRefFrom<typeof subscription>;
export type ReadRef = ActorRefFrom<typeof keyedRead>;

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

function childOf<K extends "edits" | "journeys" | "subscription" | "deployment" | "viewer">(root: SessionRootRef, id: K) {
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
export const viewerOf = (root: SessionRootRef): ReadRef => childOf(root, "viewer");
