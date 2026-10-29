// ARCHITECTURE, Web UI: edits. The edit manager owns every write the tab has sent: one write
// actor per operation id (the patch id, H5), started by the control that sent it but never
// owned by it, so a control that goes away drops its view of the write and never the write.
// A write lives in memory only: a reload loses it, and the browser asks first while any write
// is busy (`pendingOf`). The sync chip reads what did not land from here; Discard is an event sent to the
// write that owns the problem, and an unrelated save never resolves another write's.
//
// Nothing is discovered through XState's system registry: the writes are an explicit keyed
// map, each with the last snapshot it relayed (a selector reads that, never `getSnapshot()` of
// a child, so a change of a write is a change of the manager's snapshot).
import type { HttpFailure } from "@cairn/client";
import { assign, enqueueActions, fromPromise, sendTo, setup, waitFor, type ActorRefFrom } from "xstate";

import { idOf, type About, type Answer, type Command, type OperationId } from "./edit-command.ts";
import type { Markdown, Patch } from "./host.ts";
import type { Problem } from "./sync.ts";
import { writeMachine, writeView, type Check, type WriteEnv, type WriteRef, type WriteSnapshot, type WriteView } from "./write-machine.ts";
import { reasonOf } from "./writes.ts";

export interface EditsInput {
  env: WriteEnv;
}

/** A write and the last snapshot it relayed. */
export interface Entry {
  ref: WriteRef;
  snapshot: WriteSnapshot;
}

export interface EditsContext extends EditsInput {
  writes: Record<OperationId, Entry>;
}

export type EditsEvent =
  | { type: "submit"; id: OperationId; command: Command; about: About; check?: Check }
  | { type: "discard"; id: string }
  | { type: "retry"; id: OperationId }
  /** The connection came back or the tab went into skew: every write looks again. */
  | { type: "wake" }
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
        idOf(sent.command) === sent.id &&
        context.writes[sent.id] === undefined &&
        !Object.values(context.writes).some((entry) => entry.snapshot.context.about.edit === sent.about.edit && busy(entry))
      );
    },
  },
  actions: {
    // A settled write of the same edit (rejected, stopped, gone) gives way to the new one.
    dropSettled: enqueueActions(({ context, event, enqueue }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      let writes = context.writes;
      for (const [id, entry] of Object.entries(context.writes)) {
        if (entry.snapshot.context.about.edit === sent.about.edit) {
          enqueue.stopChild(entry.ref);
          writes = without(writes, id);
        }
      }
      enqueue.assign({ writes });
    }),
    startWrite: assign(({ context, event, spawn }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      const ref = spawn("write", { id: sent.id, syncSnapshot: true, input: { id: sent.id, command: sent.command, about: sent.about, env: context.env } });
      return { writes: { ...context.writes, [sent.id]: { ref, snapshot: ref.getSnapshot() } } };
    }),
    // What a write relays is the snapshot the manager reads. A relay that arrives after the
    // write ended is ignored.
    relay: assign(({ context, event }) => {
      const relayed = event as Extract<EditsEvent, { type: `xstate.snapshot.${string}` }>;
      const id = relayed.type.slice(SNAPSHOT_PREFIX.length);
      const entry = context.writes[id];
      return entry === undefined ? {} : { writes: { ...context.writes, [id]: { ref: entry.ref, snapshot: relayed.snapshot } } };
    }),
    finish: enqueueActions(({ context, event, enqueue }) => {
      const id = event.type.slice(DONE_PREFIX.length);
      const entry = context.writes[id];
      if (entry !== undefined) {
        enqueue.stopChild(entry.ref);
        enqueue.assign({ writes: without(context.writes, id) });
      }
    }),
    startSent: sendTo(({ context, event }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      const entry = context.writes[sent.id];
      if (entry === undefined) {
        throw new Error(`the write ${sent.id} was not started`);
      }
      return entry.ref;
    }, ({ event }) => {
      const sent = event as Extract<EditsEvent, { type: "submit" }>;
      return sent.check === undefined ? { type: "submit" as const } : { type: "submit" as const, check: sent.check };
    }),
    forwardDiscard: enqueueActions(({ context, event, enqueue }) => {
      const sent = event as Extract<EditsEvent, { type: "discard" }>;
      const entry = context.writes[sent.id];
      if (entry !== undefined) {
        enqueue.sendTo(entry.ref, { type: "discard" });
      }
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
  },
}).createMachine({
  id: "edits",
  context: ({ input }) => ({ ...input, writes: {} }),
  on: {
    submit: { guard: "free", actions: ["dropSettled", "startWrite", "startSent"] },
    discard: { actions: "forwardDiscard" },
    retry: { actions: "forwardRetry" },
    wake: { actions: "forwardWake" },
    "xstate.snapshot.*": { actions: "relay" },
    "xstate.done.actor.*": { actions: "finish" },
  },
});

export type EditsRef = ActorRefFrom<typeof editManager>;

/**
 * Sends `patch` to the manager as the write of `about.edit` and answers the write it started,
 * or none when the manager refused it (the edit has another write out, or this id is taken).
 * The manager takes it synchronously, so the answer is known before the caller's first await.
 */
export function submitPatch(edits: EditsRef, patch: Patch, about: About, note?: Markdown): WriteRef | undefined {
  edits.send({ type: "submit", id: patch.id, command: note === undefined ? { kind: "patch", patch } : { kind: "patch", patch, note }, about });
  return edits.getSnapshot().context.writes[patch.id]?.ref;
}

/** Whether the manager holds the write `id`: it accepted it, and it has not ended and gone. */
export function held(edits: EditsRef, id: OperationId): boolean {
  return edits.getSnapshot().context.writes[id] !== undefined;
}

/** The write of `edit`, if the manager holds one (a settled one is replaced by the next submit, so at most one). */
export function writeOf(writes: EditsContext["writes"], edit: string): Entry | undefined {
  return Object.values(writes).find((entry) => entry.snapshot.context.about.edit === edit);
}

/** Whether any write is out or may have committed and is being resent or waiting for a Retry: what the browser asks about before the tab is left. */
export function pendingOf(writes: EditsContext["writes"]): boolean {
  return Object.values(writes).some(({ snapshot }) => snapshot.hasTag("busy"));
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
export function unlandedWords(view: WriteView, skewed: boolean): { kind: Problem["kind"]; message: string; detail?: HttpFailure | undefined; retry: boolean } | undefined {
  switch (view.state) {
    case "rejected":
      return { kind: view.rejection.rejection === "stale" ? "conflict" : "rejected", message: reasonOf(view.rejection), retry: false };
    case "gone":
      return { kind: "rejected", message: removedWords(view), retry: false };
    case "unconfirmed":
      return view.retrying
        ? undefined
        : { kind: "failed", message: "Could not confirm the save; it may have landed.", detail: view.failure, retry: true };
    case "stopped":
      if (view.failure !== undefined) {
        return { kind: "failed", message: "Not saved.", detail: view.failure, retry: true };
      }
      return {
        kind: "failed",
        message: skewed ? "Cairn was updated, so this change was not sent." : "Not sent: Cairn was updated before it went out.",
        retry: true,
      };
    case "idle":
    case "sending":
    case "landed":
    case "discarded":
      return undefined;
  }
}

/** The problems on the chip: the writes that need the person, in the order they were sent. */
export function problemsOf(writes: EditsContext["writes"], skewed: boolean): Problem[] {
  const found: Problem[] = [];
  for (const [id, { snapshot }] of Object.entries(writes)) {
    const view = writeView(snapshot);
    const words = unlandedWords(view, skewed);
    if (words !== undefined) {
      found.push({ id, kind: words.kind, message: words.message, detail: words.detail, label: view.about.label, address: view.state === "gone" ? undefined : view.about.address, retry: words.retry });
    }
  }
  return found;
}

/**
 * What a control sent through the manager: the edit's command, what it is about, and the
 * check to run first. `id` is the command's own (`idOf`).
 */
export interface Sent {
  id: OperationId;
  command: Command;
  about: About;
  check?: Check | undefined;
}

/**
 * Sends `sent` to the manager and waits for it to land: its answer, or none once the write
 * needs the person (rejected, stopped, gone, or out of automatic resends), was discarded, was
 * refused for another write of the edit still out, or the session closed. The write is the
 * manager's, not the caller's: aborting `signal` stops the waiting and never the write, and a
 * write that lands after the caller stopped waiting is still recorded as a save. A repeat of
 * a write already out waits on that write.
 */
export async function submitWrite(edits: EditsRef, sent: Sent, signal?: AbortSignal): Promise<Answer | undefined> {
  edits.send({ type: "submit", id: sent.id, command: sent.command, about: sent.about, ...(sent.check === undefined ? {} : { check: sent.check }) });
  // The manager processed the submit synchronously: it holds the write unless another of the edit was out.
  const ref = edits.getSnapshot().context.writes[sent.id]?.ref;
  if (ref === undefined) {
    return undefined;
  }
  try {
    const snapshot = await waitFor(ref, (each) => each.status === "done" || each.hasTag("needsYou"), { timeout: Infinity, ...(signal === undefined ? {} : { signal }) });
    const view = writeView(snapshot);
    return view.state === "landed" ? view.answer : undefined;
  } catch {
    // The root stopped, or the caller stopped waiting.
    return undefined;
  }
}

/** What an edit is about, without the fields it has none of. */
export function aboutOf(edit: string, label: string | undefined, address: string | undefined): About {
  return { edit, ...(label === undefined ? {} : { label }), ...(address === undefined ? {} : { address }) };
}

/**
 * A workflow's delivery step: `submitWrite` as an actor, whose abort signal is the workflow's
 * own stopping (an unmount): the step stops waiting for the answer, and the write goes on.
 */
export const deliverWrite = fromPromise<Answer | undefined, { edits: EditsRef; sent: Sent }>(({ input, signal }) => submitWrite(input.edits, input.sent, signal));

/** Why a write that did not land needs the person, for a control that has no more to say than that. */
export function unansweredOf(view: WriteView | undefined): "failed" | "stopped" | "gone" | undefined {
  switch (view?.state) {
    case "unconfirmed":
      return view.retrying ? undefined : "failed";
    case "stopped":
      return "stopped";
    case "gone":
      return "gone";
    default:
      return undefined;
  }
}
