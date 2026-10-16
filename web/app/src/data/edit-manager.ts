// ARCHITECTURE, Web UI: edits. The edit manager owns every write the tab has sent: one write
// actor per operation id (the patch id, H5), started by the control that sent it but never
// owned by it, so a control that goes away drops its view of the write and never the write.
// It keeps a versioned record of each write that may still matter in session storage, so a
// reload loses nothing that may have committed, and puts the records back as writes on
// restore. The sync chip reads what did not land from here; Discard is an event sent to the
// write that owns the problem, and an unrelated save never resolves another write's.
//
// Nothing is discovered through XState's system registry: the writes are an explicit keyed
// map, each with the last snapshot it relayed (a selector reads that, never `getSnapshot()` of
// a child, so a change of a write is a change of the manager's snapshot).
import { assign, enqueueActions, sendTo, setup } from "xstate";

import { EDIT_RECORD_VERSION, type About, type Command, type EditStore, type OperationId } from "./edit-record.ts";
import type { Problem } from "./sync.ts";
import { writeMachine, writeView, recordOf, type WriteEnv, type WriteRef, type WriteSnapshot, type WriteView } from "./write-machine.ts";
import { reasonOf, unconfirmed } from "./writes.ts";

/** What the manager tells the person about the records, in words the session supplies. */
export interface EditNotes {
  /** Records of another version, or unreadable ones, were dropped on restore. */
  dropped(count: number): void;
  /** Storage refused a record: the edits are kept only until reload. */
  unstored(): void;
}

export interface EditsInput {
  store: EditStore;
  env: WriteEnv;
  notes: EditNotes;
}

/** A write and the last snapshot it relayed. */
export interface Entry {
  ref: WriteRef;
  snapshot: WriteSnapshot;
}

/** A problem a control still reports itself, until its write moves onto the manager (see the remaining-writes step). */
interface Reported {
  problem: Omit<Problem, "id" | "retry">;
  discard: () => void;
}

export interface EditsContext extends EditsInput {
  writes: Record<OperationId, Entry>;
  reported: Record<string, Reported>;
  /** Whether storage has refused a record (said once). */
  unstored: boolean;
  /** Whether the kept records have been put back: once per manager. */
  restored: boolean;
}

export type EditsEvent =
  /** Puts the kept records back as writes. Sent once, after the session root has started. */
  | { type: "restore" }
  | { type: "submit"; id: OperationId; command: Command; about: About }
  | { type: "discard"; id: string }
  | { type: "retry"; id: OperationId }
  /** The connection came back or the tab went into skew: every write looks again. */
  | { type: "wake" }
  | { type: "report"; key: string; problem: Reported["problem"]; discard: () => void }
  | { type: "resolve"; key: string }
  /** A write's snapshot, relayed by XState on each change of an active write. */
  | { type: `xstate.snapshot.${string}`; snapshot: WriteSnapshot }
  /** A write ended: it landed or was discarded. */
  | { type: `xstate.done.actor.${string}` };

const SNAPSHOT_PREFIX = "xstate.snapshot.";
const DONE_PREFIX = "xstate.done.actor.";

/** A write that is out, or may have committed and is being resent. */
const busy = (entry: Entry): boolean => entry.ref.getSnapshot().hasTag("busy");

const without = <T>(record: Record<string, T>, key: string): Record<string, T> => Object.fromEntries(Object.entries(record).filter(([each]) => each !== key));

export const editManager = setup({
  types: { context: {} as EditsContext, events: {} as EditsEvent, input: {} as EditsInput },
  actors: { write: writeMachine },
  guards: {
    // One busy write per edit: a second submit before the first is answered is refused.
    free: ({ context, event }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      return (
        sent.command.patch.id === sent.id &&
        context.writes[sent.id] === undefined &&
        !Object.values(context.writes).some((entry) => entry.snapshot.context.about.edit === sent.about.edit && busy(entry))
      );
    },
    notRestored: ({ context }) => !context.restored,
  },
  actions: {
    // The kept records come back as writes. Reading the store and saying what it dropped are
    // effects of this one step. The writes read the session root as they start, so this runs
    // on an event after the root has started, never in the manager's initial context.
    restoreWrites: assign(({ context, spawn }) => {
      const { records, dropped } = context.store.read();
      if (dropped > 0) {
        context.notes.dropped(dropped);
      }
      const writes: Record<OperationId, Entry> = {};
      for (const record of records) {
        const ref = spawn("write", {
          id: record.id,
          syncSnapshot: true,
          input: { id: record.id, command: record.command, about: record.about, env: context.env, restored: { delivery: record.delivery, resubmitted: record.resubmitted } },
        });
        writes[record.id] = { ref, snapshot: ref.getSnapshot() };
      }
      return { writes, restored: true };
    }),
    // A settled write of the same edit (rejected, stopped, gone) gives way to the new one.
    dropSettled: enqueueActions(({ context, event, enqueue }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      let writes = context.writes;
      for (const [id, entry] of Object.entries(context.writes)) {
        if (entry.snapshot.context.about.edit === sent.about.edit) {
          enqueue.stopChild(entry.ref);
          context.store.forget(id);
          writes = without(writes, id);
        }
      }
      enqueue.assign({ writes });
    }),
    startWrite: assign(({ context, event, spawn }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      // The record is kept before the request goes out (the write sends it on the submit that
      // follows), or a crash between the two would lose the id of an edit that may have committed.
      const kept = context.store.keep({ version: EDIT_RECORD_VERSION, id: sent.id, command: sent.command, about: sent.about, delivery: { state: "sending" }, resubmitted: 0 });
      const ref = spawn("write", { id: sent.id, syncSnapshot: true, input: { id: sent.id, command: sent.command, about: sent.about, env: context.env, restored: undefined } });
      if (!kept && !context.unstored) {
        context.notes.unstored();
      }
      return { writes: { ...context.writes, [sent.id]: { ref, snapshot: ref.getSnapshot() } }, unstored: context.unstored || !kept };
    }),
    // What a write relays is kept as its record when it is one worth keeping. A relay that
    // arrives after the write ended is ignored, or the record would come back on reload.
    relay: assign(({ context, event }) => {
      const relayed = event as Extract<EditsEvent, { type: `xstate.snapshot.${string}` }>;
      const id = relayed.type.slice(SNAPSHOT_PREFIX.length);
      const entry = context.writes[id];
      if (entry === undefined) {
        return {};
      }
      const record = recordOf(writeView(relayed.snapshot));
      let { unstored } = context;
      if (record !== undefined && !context.store.keep(record) && !unstored) {
        context.notes.unstored();
        unstored = true;
      }
      return { writes: { ...context.writes, [id]: { ref: entry.ref, snapshot: relayed.snapshot } }, unstored };
    }),
    finish: enqueueActions(({ context, event, enqueue }) => {
      const id = event.type.slice(DONE_PREFIX.length);
      const entry = context.writes[id];
      if (entry !== undefined) {
        enqueue.stopChild(entry.ref);
        enqueue.assign({ writes: without(context.writes, id) });
      }
      context.store.forget(id);
    }),
    startSent: sendTo(({ context, event }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      const entry = context.writes[sent.id];
      if (entry === undefined) {
        throw new Error(`the write ${sent.id} was not started`);
      }
      return entry.ref;
    }, { type: "submit" }),
    forwardDiscard: enqueueActions(({ context, event, enqueue }) => {
      const sent = event as Extract<EditsEvent, { type: "discard" }>;
      const entry = context.writes[sent.id];
      if (entry !== undefined) {
        enqueue.sendTo(entry.ref, { type: "discard" });
        return;
      }
      // A problem a control reports itself drops with the control's own discard.
      context.reported[sent.id]?.discard();
      enqueue.assign({ reported: without(context.reported, sent.id) });
    }),
    forwardRetry: enqueueActions(({ context, event, enqueue }) => {
      const sent = event as Extract<EditsEvent, { type: "retry" }>;
      const entry = context.writes[sent.id];
      if (entry !== undefined) {
        enqueue.sendTo(entry.ref, { type: "retry" });
      }
    }),
    forwardWake: enqueueActions(({ context, enqueue }) => {
      for (const entry of Object.values(context.writes)) {
        enqueue.sendTo(entry.ref, { type: "wake" });
      }
    }),
    report: assign(({ context, event }) => {
      const sent = event as Extract<EditsEvent, { type: "report" }>;
      return { reported: { ...context.reported, [sent.key]: { problem: sent.problem, discard: sent.discard } } };
    }),
    resolve: assign(({ context, event }) => ({ reported: without(context.reported, (event as Extract<EditsEvent, { type: "resolve" }>).key) })),
  },
}).createMachine({
  id: "edits",
  context: ({ input }) => ({ ...input, writes: {}, reported: {}, unstored: false, restored: false }),
  on: {
    restore: { guard: "notRestored", actions: "restoreWrites" },
    submit: { guard: "free", actions: ["dropSettled", "startWrite", "startSent"] },
    discard: { actions: "forwardDiscard" },
    retry: { actions: "forwardRetry" },
    wake: { actions: "forwardWake" },
    report: { actions: "report" },
    resolve: { actions: "resolve" },
    "xstate.snapshot.*": { actions: "relay" },
    "xstate.done.actor.*": { actions: "finish" },
  },
});

/** The write of `edit`, if the manager holds one (a settled one is replaced by the next submit, so at most one). */
export function writeOf(writes: EditsContext["writes"], edit: string): Entry | undefined {
  return Object.values(writes).find((entry) => entry.snapshot.context.about.edit === edit);
}

/** Writes out or being resent: what the chip counts as saving. A write that stopped resending needs the person instead. */
export function inFlightOf(writes: EditsContext["writes"]): number {
  return Object.values(writes).filter(({ snapshot }) => snapshot.hasTag("busy") && !snapshot.hasTag("needsYou")).length;
}

const removedWords = (view: Extract<WriteView, { state: "gone" }>): string => {
  if ("journey" in view.removed) {
    return "The journey was deleted, so this change cannot be saved.";
  }
  return "route" in view.removed ? "The route was deleted, so this change cannot be saved." : "This step was removed, so this change cannot be saved.";
};

/**
 * What the chip says of a write that needs the person, or none for one that does not (out, or
 * still resending, which the chip counts as saving). `skewed`: the tab is in version skew now.
 */
export function unlandedWords(view: WriteView, skewed: boolean): { kind: Problem["kind"]; message: string; retry: boolean } | undefined {
  switch (view.state) {
    case "rejected":
      return { kind: view.rejection.rejection === "stale" ? "conflict" : "rejected", message: reasonOf(view.rejection), retry: false };
    case "gone":
      return { kind: "rejected", message: removedWords(view), retry: false };
    case "unconfirmed":
      return view.retrying
        ? undefined
        : { kind: "failed", message: view.failure === undefined ? "Could not confirm the save: the page reloaded before the answer came." : unconfirmed(view.failure), retry: true };
    case "stopped":
      if (view.failure !== undefined) {
        return { kind: "failed", message: `Not saved: ${view.failure.message}.`, retry: true };
      }
      return {
        kind: "failed",
        message: skewed ? "Cairn was updated, so this change was not sent. Reload; your unsent edits are kept." : "Not sent: Cairn was updated before it went out.",
        retry: true,
      };
    case "idle":
    case "sending":
    case "landed":
    case "discarded":
      return undefined;
  }
}

/** The problems on the chip: the writes that need the person in the order they were sent, then those a control still reports. */
export function problemsOf(writes: EditsContext["writes"], reported: EditsContext["reported"], skewed: boolean): Problem[] {
  const found: Problem[] = [];
  for (const [id, { snapshot }] of Object.entries(writes)) {
    const view = writeView(snapshot);
    const words = unlandedWords(view, skewed);
    if (words !== undefined) {
      found.push({ id, kind: words.kind, message: words.message, label: view.about.label, address: view.state === "gone" ? undefined : view.about.address, retry: words.retry });
    }
  }
  for (const [key, { problem }] of Object.entries(reported)) {
    found.push({ ...problem, id: key, retry: false });
  }
  return found;
}
