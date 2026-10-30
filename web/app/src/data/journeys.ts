// The journeys a tab shows, each derived in the derive worker (ARCHITECTURE, Web UI: data
// flow): one fetch of the domain document, posted to the worker once per revision, derived
// there, and its derive read back. The journey manager, a child of the session root, keeps one
// keyed read per journey in an explicit map, and keeps the views current (H6): a journey tick
// newer than the revision held refetches it, as does a deployment tick newer than the
// deployment its document was derived with (owners and identities feed rank) and the date passing
// in the deployment's zone. A document from another engine (`Skewed`) latches the tab into skew
// instead of being derived. Revisions only move forward: an older document is never shown.
//
// A derivation is owned by the load that made it (the worker numbers loads): the read that
// derived a document holds its handle, and the handle is released when the read drops the value,
// so an owner that was evicted and a successor that replaced it never free each other's.
import type { RevisionOf, Schema } from "@cairn/client";
import type { DerivationHandle, DerivationKey, Derived } from "@cairn/wasm";
import { assign, enqueueActions, setup, type ActorRefFrom, type SnapshotFrom } from "xstate";

import type { Deriver, DomainDocument, Host } from "./host.ts";
import {
  DERIVE,
  beforeWork,
  heldValue,
  keyedRead,
  readLag,
  readView,
  worstLag,
  type Lag,
  type ReadEnv,
  type ReadError,
  type ReadRef,
  type ReadSnapshot,
  type ReadSource,
} from "./keyed-read.ts";
import { HOLDS_RETRY_MS, ROLLOVER_CHECK_MS, todayIn } from "./today.ts";

export type Journey = Schema<"Journey">;
export type DeriveInputs = Schema<"DeriveInputs">;

/** What a journey's view shows. */
export type JourneyView =
  | { status: "loading" }
  | { status: "missing" }
  | { status: "failed"; message: string }
  | { status: "skew" }
  | { status: "ready"; journey: Journey; inputs: DeriveInputs; key: DerivationKey; derived: Derived };

/**
 * Journeys kept derived after their view closes, so going back to one shows it at once
 * (and refetches it only if it moved meanwhile); the least recently shown beyond this are
 * released from the worker.
 */
export const PARKED_JOURNEY_COUNT_MAX = 8;

/** What a journey read holds: the document, its derive, and the load that owns the derivation. */
export interface JourneyHeld {
  journey: Journey;
  inputs: DeriveInputs;
  key: DerivationKey;
  derived: Derived;
  handle: DerivationHandle;
  revision: number;
  deployment: number;
  today: string;
  timezone: string;
}

/** What the most recently shown journey's view is derived from (the sync popover). */
export interface Derivation {
  revision: number;
  deployment: number;
  today: string;
}

export const journeyOf = (id: string): RevisionOf => ({ domain: { journey: id } });

const DEPLOYMENT: RevisionOf = { domain: "deployment" };

/**
 * The journey `args.id`'s read: its document, posted to the worker and derived. Obsolete work
 * never publishes over its successor's: the read posts a load only while the machine still waits
 * for it and the tab is compatible, owns the load until the machine adopts the value, and frees it
 * itself if the answer is dropped or the derive fails.
 */
export function journeySource(host: Host, deriver: Deriver): ReadSource<{ id: string }, JourneyHeld> {
  return {
    policy: DERIVE,
    async read({ id }, at) {
      const text = await host.documentText(id);
      const document = JSON.parse(text) as DomainDocument;
      const revision = document.journey.revision;
      const deployment = document.inputs.deployment.revision;
      const held = at.held;
      if (held !== undefined && (revision < held.revision || deployment < held.deployment)) {
        return held;
      }
      beforeWork(at);
      const { key, handle } = await deriver.load(text);
      let freed = false;
      const free = () => {
        if (!freed) {
          freed = true;
          void deriver.release(handle).catch(() => undefined);
        }
      };
      // The abort fires exactly when the answer was not and will not be adopted.
      at.signal.addEventListener("abort", free, { once: true });
      try {
        beforeWork(at);
        const derived = await deriver.derived(id);
        const { today, timezone } = document.inputs;
        return { journey: document.journey, inputs: document.inputs, key, derived, handle, revision, deployment, today, timezone };
      } catch (thrown) {
        free();
        throw thrown;
      }
    },
    ticks: ({ id }) => ({
      watching: [`journey:${id}`, "deployment"],
      about: (of) => !("proposal" in of) && (of.domain === "deployment" || ("journey" in of.domain && of.domain.journey === id)),
      holds: (value) => [
        { of: journeyOf(id), revision: value.revision },
        { of: DEPLOYMENT, revision: value.deployment },
      ],
      subject: journeyOf(id),
    }),
    release(value) {
      void deriver.release(value.handle).catch(() => undefined);
    },
  };
}

/** A journey read and the last snapshot it relayed. */
export interface JourneyEntry {
  ref: ReadRef;
  snapshot: ReadSnapshot;
  mounted: number;
  used: number;
}

export interface JourneysInput {
  host: Host;
  deriver: Deriver;
  env: ReadEnv;
  /** The page's clock, for date rollover. */
  today(): Date;
}

export interface JourneysContext extends JourneysInput {
  entries: Readonly<Record<string, JourneyEntry>>;
  uses: number;
}

export type JourneysEvent =
  | { type: "mount"; id: string }
  | { type: "unmount"; id: string }
  | { type: "retry"; id: string }
  | { type: "holds"; id: string }
  | { type: `xstate.snapshot.${string}`; snapshot: ReadSnapshot };

const SNAPSHOT_PREFIX = "xstate.snapshot.";
const idOf = (event: JourneysEvent): string => (event as Extract<JourneysEvent, { type: "mount" }>).id;

export const journeyManager = setup({
  types: { context: {} as JourneysContext, events: {} as JourneysEvent, input: {} as JourneysInput },
  actors: { read: keyedRead },
  actions: {
    // The journey is shown once more: its read is started on the first mount.
    mount: assign(({ context, event, spawn }) => {
      const id = idOf(event);
      let entry = context.entries[id];
      if (entry === undefined) {
        const ref = spawn("read", { id, syncSnapshot: true, input: { source: journeySource(context.host, context.deriver), env: context.env } });
        entry = { ref, snapshot: ref.getSnapshot(), mounted: 0, used: 0 };
      }
      const uses = context.uses + 1;
      return { entries: { ...context.entries, [id]: { ...entry, mounted: entry.mounted + 1, used: uses } }, uses };
    }),
    // A new read is asked for its journey; a parked one listens again, and looks again if it holds nothing to show.
    greet: enqueueActions(({ context, event, enqueue }) => {
      const id = idOf(event);
      const entry = context.entries[id];
      if (entry === undefined) {
        return;
      }
      if (entry.ref.getSnapshot().matches("idle")) {
        enqueue.sendTo(entry.ref, { type: "want", key: { of: [id], freshness: [], args: { id } } });
        return;
      }
      enqueue.sendTo(entry.ref, { type: "show" });
      if (heldValue(entry.snapshot) === undefined) {
        enqueue.sendTo(entry.ref, { type: "refresh" });
      }
    }),
    // The person asked again for a journey that could not be read: its read looks again.
    retry: enqueueActions(({ context, event, enqueue }) => {
      const entry = context.entries[idOf(event)];
      if (entry !== undefined) {
        enqueue.sendTo(entry.ref, { type: "refresh" });
      }
    }),
    park: enqueueActions(({ context, event, enqueue }) => {
      const id = idOf(event);
      const entry = context.entries[id];
      if (entry === undefined || entry.mounted === 0) {
        return;
      }
      const mounted = entry.mounted - 1;
      let entries: Record<string, JourneyEntry> = { ...context.entries, [id]: { ...entry, mounted } };
      if (mounted === 0) {
        enqueue.sendTo(entry.ref, { type: "park" });
        // The least recently shown parked journeys beyond the bound are released from the worker.
        const parked = Object.entries(entries)
          .filter(([, each]) => each.mounted === 0)
          .sort(([, left], [, right]) => right.used - left.used);
        for (const [evicted, each] of parked.slice(PARKED_JOURNEY_COUNT_MAX)) {
          enqueue.stopChild(each.ref);
          entries = Object.fromEntries(Object.entries(entries).filter(([name]) => name !== evicted));
        }
      }
      enqueue.assign({ entries });
    }),
    // A derivation holds until the next time of day that would change it: asking again is
    // scheduled for that moment, once for each derivation adopted (a failed read keeps the
    // one it has, and retries on the read's own terms).
    relay: enqueueActions(({ context, event, enqueue }) => {
      const relayed = event as Extract<JourneysEvent, { type: `xstate.snapshot.${string}` }>;
      const id = relayed.type.slice(SNAPSHOT_PREFIX.length);
      const entry = context.entries[id];
      if (entry === undefined) {
        return;
      }
      enqueue.assign({ entries: { ...context.entries, [id]: { ...entry, snapshot: relayed.snapshot } } });
      const adopted = heldValue<JourneyHeld>(relayed.snapshot);
      if (adopted === undefined || adopted === heldValue<JourneyHeld>(entry.snapshot)) {
        return;
      }
      const holds = adopted.derived.holds_until;
      enqueue.cancel(`holds:${id}`);
      if (holds != null) {
        enqueue.raise({ type: "holds", id }, { id: `holds:${id}`, delay: Math.max(Date.parse(holds) - context.today().getTime(), HOLDS_RETRY_MS) });
      }
    }),
    // Asked at each check until the host's today catches up with the page's clock.
    rollover: enqueueActions(({ context, enqueue }) => {
      for (const entry of Object.values(context.entries)) {
        const held = heldValue<JourneyHeld>(entry.snapshot);
        if (held === undefined) {
          continue;
        }
        try {
          if (todayIn(held.timezone, context.today()) !== held.today) {
            enqueue.sendTo(entry.ref, { type: "refresh" });
          }
        } catch {
          // A zone the page cannot name keeps its day.
        }
      }
    }),
  },
}).createMachine({
  id: "journeys",
  context: ({ input }) => ({ ...input, entries: {}, uses: 0 }),
  initial: "running",
  states: {
    // Date rollover: a check every so often, on the machine's own clock.
    running: { after: { [ROLLOVER_CHECK_MS]: { target: "running", reenter: true, actions: "rollover" } } },
  },
  on: {
    mount: { actions: ["mount", "greet"] },
    unmount: { actions: "park" },
    retry: { actions: "retry" },
    holds: { actions: "retry" },
    "xstate.snapshot.*": { actions: "relay" },
  },
});

export type JourneysRef = ActorRefFrom<typeof journeyManager>;
export type JourneysSnapshot = SnapshotFrom<typeof journeyManager>;

const LOADING: JourneyView = { status: "loading" };
export const MISSING: JourneyView = { status: "missing" };
const SKEW: JourneyView = { status: "skew" };
const ready = new WeakMap<JourneyHeld, JourneyView>();
const failures = new WeakMap<ReadError, JourneyView>();

/** The view a read shows for journey `id`: the same object while what it shows holds. */
function viewOf(snapshot: ReadSnapshot, id: string): JourneyView {
  const view = readView<JourneyHeld>(snapshot, [id], []);
  switch (view.status) {
    case "current":
    case "refreshing":
    case "behind": {
      let found = ready.get(view.value);
      if (found === undefined) {
        const { journey, inputs, key, derived } = view.value;
        found = { status: "ready", journey, inputs, key, derived };
        ready.set(view.value, found);
      }
      return found;
    }
    case "gone":
      return MISSING;
    case "failed": {
      if (view.error.kind === "skew") {
        return SKEW;
      }
      let found = failures.get(view.error);
      if (found === undefined) {
        found = { status: "failed", message: view.error.message };
        failures.set(view.error, found);
      }
      return found;
    }
    case "loading":
    case "idle":
      return LOADING;
  }
}

/** What journey `id`'s view shows. A malformed id is read like any other and found missing. */
export function journeyView(snapshot: JourneysSnapshot, id: string): JourneyView {
  const entry = snapshot.context.entries[id];
  return entry === undefined ? LOADING : viewOf(entry.snapshot, id);
}

/** The mounted journey worst behind what the stream announced (a failed refetch first, then the longest wait), if any. */
export function lagOf(snapshot: JourneysSnapshot): Lag | undefined {
  return worstLag(
    Object.values(snapshot.context.entries)
      .filter((entry) => entry.mounted > 0)
      .map((entry) => readLag(entry.snapshot)),
  );
}

/** What the most recently shown journey is at, if one is shown. */
export function derivationOf(snapshot: JourneysSnapshot): Derivation | undefined {
  let latest: JourneyEntry | undefined;
  for (const entry of Object.values(snapshot.context.entries)) {
    if (entry.mounted > 0 && heldValue(entry.snapshot) !== undefined && (latest === undefined || entry.used > latest.used)) {
      latest = entry;
    }
  }
  const held = latest === undefined ? undefined : heldValue<JourneyHeld>(latest.snapshot);
  return held === undefined ? undefined : { revision: held.revision, deployment: held.deployment, today: held.today };
}

/** Whether two derivations are at the same point: a selector compares with this so React re-renders only on a change. */
export function sameDerivation(left: Derivation | undefined, right: Derivation | undefined): boolean {
  return left?.revision === right?.revision && left?.deployment === right?.deployment && left?.today === right?.today;
}
