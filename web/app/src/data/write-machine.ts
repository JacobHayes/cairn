// ARCHITECTURE, Web UI: edits. The delivery of one edit, from the moment a control sends it
// to the moment it lands or is discarded (H5). A write is one actor, owned by the edit
// manager and keyed by its patch id, never by the control that sent it: its payload, its
// rejection, its uncertainty and its recovery live here.
//
//   idle -> sending -> landed                     the commit was acknowledged
//                   -> rejected                   a definite refusal, with its violations
//                   -> gone                       the edit's subject no longer exists
//                   -> stopped                    refused or not sent: known not committed
//                   -> unconfirmed                no answer: it may have committed
//
// `unconfirmed` resends itself with the same patch id and the last patch it sent, which is
// safe because the API is idempotent on the patch id (H5): `already_applied` settles it as
// landed, and a normal answer settles it either way. It resends at once on restore after a
// reload, and otherwise after a backoff; each attempt waits for the connection, so time
// offline uses none; it stops after `RESEND_COUNT_MAX` attempts and asks the person to
// Retry. Version skew stops it, as it stops all new work.
import {
  RESUBMISSION_COUNT_MAX,
  backoffDelayMs,
  networkFailure,
  sameDomain,
  submit,
  targetDomain,
  type Answered,
  type HttpFailure,
  type Overlaps,
  type Patch,
  type PatchAnswer,
  type Rejection,
  type Send,
  type Submission,
} from "@cairn/client";
import { assign, fromPromise, setup, type ActorRefFrom, type SnapshotFrom } from "xstate";

import type { About, Command, Delivery, EditRecord, OperationId, Removed } from "./edit-record.ts";
import { EDIT_RECORD_VERSION } from "./edit-record.ts";
import type { Markdown } from "./host.ts";
import { subjectOf } from "./writes.ts";

/**
 * Automatic resends of a write that got no answer, at most (PRACTICES, Explicit limits): with
 * the backoff, five cover about eight seconds of a server that answers nothing. Past them the
 * person retries by hand.
 */
export const RESEND_COUNT_MAX = 5;

/** What a write reads and does outside itself; the session supplies it. */
export interface WriteEnv {
  /** Sends one patch (bound to the host), its events carrying `note`. */
  send: (patch: Patch, note?: Markdown) => Promise<Answered<HttpFailure>>;
  overlaps: Overlaps;
  /** Counts a write being sent (D7: a save made before a screen opened is not that screen's). */
  began(): number;
  /** Records a landed write as a save (D7). */
  saved(command: Command, answer: PatchAnswer, began: number): void;
  /** Whether the tab is not in version skew. */
  compatible(): boolean;
  /** Whether the connection is back: the browser is online and the tick stream is open. */
  connected(): boolean;
}

export interface WriteInput {
  id: OperationId;
  command: Command;
  about: About;
  env: WriteEnv;
  /** What a kept record says, when the write is restored after a reload. */
  restored: { delivery: Delivery; resubmitted: number } | undefined;
}

interface WriteContext {
  id: OperationId;
  command: Command;
  about: About;
  env: WriteEnv;
  /** The count `env.began` gave when the write first went out, for D7. */
  began: number | undefined;
  /** Automatic resends so far in this stretch of uncertainty. */
  resent: number;
  /** Stale answers rebased and resubmitted by the safe retry, across every send of this write. */
  resubmitted: number;
  restored: Delivery["state"] | undefined;
  rejection: Rejection | undefined;
  failure: HttpFailure | undefined;
  removed: Removed | undefined;
  answer: PatchAnswer | undefined;
}

type WriteEvent =
  | { type: "submit" }
  | { type: "retry" }
  | { type: "discard" }
  /** The connection came back or the tab went into skew: whatever waits looks again. */
  | { type: "wake" }
  /** The safe retry rebased the patch: the write now holds, and the record keeps, what it sent last. */
  | { type: "dispatched"; patch: Patch };

interface DeliverInput {
  patch: Patch;
  note: Markdown | undefined;
  resubmitted: number;
  env: WriteEnv;
  dispatched: (patch: Patch) => void;
}

/**
 * H5: sends the patch through the client's safe retry. The retry bound is the write's, not the
 * call's (it carries across resends and reloads), and nothing is rebased once the write
 * stopped waiting or the tab is in skew. A rebased patch is reported before it goes out, so
 * the record keeps what may have committed.
 */
const deliver = fromPromise<Submission<HttpFailure>, DeliverInput>(({ input, signal }) => {
  let resubmitted = input.resubmitted;
  let first = true;
  const send: Send<HttpFailure> = (patch) => {
    if (!first) {
      resubmitted += 1;
      input.dispatched(patch);
    }
    first = false;
    return input.env.send(patch, input.note);
  };
  return submit(input.patch, send, {
    overlaps: input.env.overlaps,
    mayRetry: () => !signal.aborted && input.env.compatible() && resubmitted < RESUBMISSION_COUNT_MAX,
  });
});

/** What the delivery is given: the write's last patch, and how to report one it rebases. */
function deliverInput({ context, self }: { context: WriteContext; self: { send: (event: WriteEvent) => void } }): DeliverInput {
  return {
    patch: context.command.patch,
    note: context.command.note,
    resubmitted: context.resubmitted,
    env: context.env,
    dispatched: (patch) => {
      self.send({ type: "dispatched", patch });
    },
  };
}

/**
 * A18, A19: what a definite refusal says no longer exists, from the shapes the engine answers.
 * A stale answer naming the write's own journey or route at revision 0 (the engine's revision
 * of a domain it does not hold; the base is checked first, so a deleted domain answers stale,
 * never `target_missing`); or a `dangling_reference` at the mutation's own node with nothing
 * related (the shape of a mutation about a node the graph does not hold). A live node that
 * points to a removed one is `related` to it, so it stays a rejection.
 */
export function goneOf(patch: Patch, rejection: Rejection): Removed | undefined {
  if (rejection.rejection === "stale") {
    const own = targetDomain(patch.target);
    if (own === undefined || own === "deployment") {
      return undefined;
    }
    const missing = rejection.conflicts.some((conflict) => conflict.current === 0 && "domain" in conflict.of && sameDomain(conflict.of.domain, own));
    return missing ? ("journey" in own ? { journey: own.journey } : { route: own.route }) : undefined;
  }
  if (rejection.rejection !== "invalid") {
    return undefined;
  }
  for (const violation of rejection.violations) {
    const { mutation, subject } = violation.at;
    const mine = mutation === undefined || mutation === null ? undefined : patch.mutations[mutation];
    if (
      violation.code === "dangling_reference" &&
      mine !== undefined &&
      (violation.related ?? []).length === 0 &&
      subject !== undefined &&
      subject !== null &&
      typeof subject === "object" &&
      "node" in subject &&
      subject.node === subjectOf(mine)
    ) {
      return { node: subject.node };
    }
  }
  return undefined;
}

type Outcome = Submission<HttpFailure>;
const thrownOf = ({ event }: { event: { error: unknown } }): unknown => event.error;
const outcomeOf = ({ event }: { event: { output: Outcome } }): Outcome => event.output;

/** A refusal before anything committed: a 4xx answer to a request that reached the server. */
const refused = (failure: HttpFailure): boolean => failure.status >= 400 && failure.status < 500;

export const writeMachine = setup({
  types: {
    context: {} as WriteContext,
    events: {} as WriteEvent,
    input: {} as WriteInput,
  },
  actors: { deliver },
  delays: { backoff: ({ context }) => backoffDelayMs(context.resent) },
  guards: {
    compatible: ({ context }) => context.env.compatible(),
    incompatible: ({ context }) => !context.env.compatible(),
    connected: ({ context }) => context.env.connected(),
    exhausted: ({ context }) => context.resent >= RESEND_COUNT_MAX,
    restoredAs: ({ context }, state: Delivery["state"]) => context.restored === state,
    landed: (_, outcome: Outcome) => outcome.outcome === "landed",
    gone: ({ context }, outcome: Outcome) => outcome.outcome === "rejected" && goneOf(context.command.patch, outcome.rejection) !== undefined,
    rejected: (_, outcome: Outcome) => outcome.outcome === "rejected",
    refused: (_, outcome: Outcome) => outcome.outcome === "failed" && refused(outcome.error),
  },
  actions: {
    begin: assign({ began: ({ context }) => context.began ?? context.env.began() }),
    resend: assign({ resent: ({ context }) => context.resent + 1 }),
    freshBudget: assign({ resent: 0 }),
    clearFailure: assign({ failure: undefined }),
    // Every answer leaves what it said in the context, whichever state it moves the write to.
    take: assign(({ context }, outcome: Outcome) => ({
      answer: outcome.outcome === "landed" ? outcome.answer : undefined,
      rejection: outcome.outcome === "rejected" ? outcome.rejection : undefined,
      failure: outcome.outcome === "failed" ? outcome.error : undefined,
      removed: outcome.outcome === "rejected" ? goneOf(context.command.patch, outcome.rejection) : undefined,
    })),
    takeError: assign({ failure: (_, thrown: unknown) => networkFailure(thrown) }),
    rebased: assign(({ context }, patch: Patch) => ({
      command: { ...context.command, patch },
      resubmitted: context.resubmitted + 1,
    })),
    save: ({ context }) => {
      if (context.answer !== undefined && context.began !== undefined) {
        context.env.saved(context.command, context.answer, context.began);
      }
    },
  },
}).createMachine({
  id: "write",
  context: ({ input }) => ({
    id: input.id,
    command: input.command,
    about: input.about,
    env: input.env,
    began: undefined,
    resent: 0,
    resubmitted: input.restored?.resubmitted ?? 0,
    restored: input.restored?.delivery.state,
    rejection: input.restored?.delivery.state === "rejected" ? input.restored.delivery.rejection : undefined,
    failure: input.restored?.delivery.state === "unconfirmed" || input.restored?.delivery.state === "stopped" ? input.restored.delivery.failure : undefined,
    removed: input.restored?.delivery.state === "gone" ? input.restored.delivery.removed : undefined,
    answer: undefined,
  }),
  initial: "start",
  on: { dispatched: { actions: { type: "rebased", params: ({ event }: { event: Extract<WriteEvent, { type: "dispatched" }> }) => event.patch } } },
  states: {
    start: {
      always: [
        // What may have committed is resent at once: the reload is the person's retry.
        { guard: { type: "restoredAs", params: "sending" }, target: "unconfirmed.ready" },
        { guard: { type: "restoredAs", params: "unconfirmed" }, target: "unconfirmed.ready" },
        { guard: { type: "restoredAs", params: "rejected" }, target: "rejected" },
        { guard: { type: "restoredAs", params: "stopped" }, target: "stopped" },
        { guard: { type: "restoredAs", params: "gone" }, target: "gone" },
        { target: "idle" },
      ],
    },
    idle: {
      on: {
        submit: [{ guard: "compatible", target: "sending" }, { target: "stopped" }],
      },
    },
    // A busy write does not accept a second submit.
    sending: {
      tags: ["busy"],
      entry: "begin",
      invoke: {
        src: "deliver",
        input: deliverInput,
        onDone: [
          { guard: { type: "landed", params: outcomeOf }, target: "landed", actions: { type: "take", params: outcomeOf } },
          { guard: { type: "gone", params: outcomeOf }, target: "gone", actions: { type: "take", params: outcomeOf } },
          { guard: { type: "rejected", params: outcomeOf }, target: "rejected", actions: { type: "take", params: outcomeOf } },
          { guard: { type: "refused", params: outcomeOf }, target: "stopped", actions: { type: "take", params: outcomeOf } },
          { target: "unconfirmed", actions: { type: "take", params: outcomeOf } },
        ],
        onError: { target: "unconfirmed", actions: { type: "takeError", params: thrownOf } },
      },
    },
    // Known not to have committed: what a 4xx refusal or a tab in skew did not send.
    stopped: {
      tags: ["needsYou", "settled"],
      on: {
        retry: { guard: "compatible", target: "sending", actions: "clearFailure" },
        discard: "discarded",
      },
    },
    unconfirmed: {
      tags: ["busy"],
      initial: "backoff",
      on: { discard: "discarded" },
      states: {
        backoff: {
          after: { backoff: "ready" },
          on: { wake: { guard: "incompatible", target: "manual" } },
        },
        // Looks at skew, the budget and the connection, and sends, waits or gives up.
        ready: {
          always: [
            { guard: "incompatible", target: "manual" },
            { guard: "exhausted", target: "manual" },
            { guard: "connected", target: "resending" },
            { target: "waiting" },
          ],
        },
        waiting: { on: { wake: "ready" } },
        resending: {
          entry: ["begin", "resend"],
          invoke: {
            src: "deliver",
            input: deliverInput,
            onDone: [
              { guard: { type: "landed", params: outcomeOf }, target: "#write.landed", actions: { type: "take", params: outcomeOf } },
              { guard: { type: "gone", params: outcomeOf }, target: "#write.gone", actions: { type: "take", params: outcomeOf } },
              { guard: { type: "rejected", params: outcomeOf }, target: "#write.rejected", actions: { type: "take", params: outcomeOf } },
              // A refusal of a resend says nothing of whether the lost attempt committed.
              { guard: { type: "refused", params: outcomeOf }, target: "manual", actions: { type: "take", params: outcomeOf } },
              { guard: "exhausted", target: "manual", actions: { type: "take", params: outcomeOf } },
              { target: "backoff", actions: { type: "take", params: outcomeOf } },
            ],
            onError: [
              { guard: "exhausted", target: "manual", actions: { type: "takeError", params: thrownOf } },
              { target: "backoff", actions: { type: "takeError", params: thrownOf } },
            ],
          },
        },
        manual: {
          tags: ["needsYou"],
          on: { retry: { guard: "compatible", target: "ready", actions: "freshBudget" } },
        },
      },
    },
    // A definite refusal, with its violations and what was tried.
    rejected: { tags: ["needsYou", "settled"], on: { discard: "discarded" } },
    // The edit's subject no longer exists: only Discard is left.
    gone: { tags: ["needsYou", "settled"], on: { discard: "discarded" } },
    landed: { type: "final", entry: "save" },
    discarded: { type: "final" },
  },
});

export type WriteSnapshot = SnapshotFrom<typeof writeMachine>;
export type WriteRef = ActorRefFrom<typeof writeMachine>;

/** What a control and the chip read of a write; tagged, so none pieces it from optional fields. */
export type WriteView = {
  id: OperationId;
  about: About;
  command: Command;
  /** Stale answers rebased and resubmitted so far (retry.ts's bound, kept per write). */
  resubmitted: number;
} & (
  | { state: "idle" | "sending" | "discarded" }
  | { state: "gone"; removed: Removed }
  | { state: "unconfirmed"; failure: HttpFailure | undefined; retrying: boolean }
  | { state: "rejected"; rejection: Rejection }
  | { state: "stopped"; failure: HttpFailure | undefined }
  | { state: "landed"; answer: PatchAnswer }
);

/** The tagged view of a write's snapshot. */
export function writeView(snapshot: WriteSnapshot): WriteView {
  const { context } = snapshot;
  const common = { id: context.id, about: context.about, command: context.command, resubmitted: context.resubmitted };
  if (snapshot.matches("sending")) {
    return { ...common, state: "sending" };
  }
  if (snapshot.matches("unconfirmed")) {
    return { ...common, state: "unconfirmed", failure: context.failure, retrying: !snapshot.matches({ unconfirmed: "manual" }) };
  }
  if (snapshot.matches("rejected") && context.rejection !== undefined) {
    return { ...common, state: "rejected", rejection: context.rejection };
  }
  if (snapshot.matches("stopped")) {
    return { ...common, state: "stopped", failure: context.failure };
  }
  if (snapshot.matches("gone") && context.removed !== undefined) {
    return { ...common, state: "gone", removed: context.removed };
  }
  if (snapshot.matches("landed") && context.answer !== undefined) {
    return { ...common, state: "landed", answer: context.answer };
  }
  return { ...common, state: snapshot.matches("discarded") ? "discarded" : "idle" };
}

/** What the tab keeps of a write: nothing for one that has not been sent, has landed or is discarded. */
export function recordOf(view: WriteView): EditRecord | undefined {
  const base: Omit<EditRecord, "delivery"> = { version: EDIT_RECORD_VERSION, id: view.id, command: view.command, about: view.about, resubmitted: view.resubmitted };
  switch (view.state) {
    case "sending":
      return { ...base, delivery: { state: "sending" } };
    case "unconfirmed":
      return { ...base, delivery: view.failure === undefined ? { state: "unconfirmed" } : { state: "unconfirmed", failure: view.failure } };
    case "rejected":
      return { ...base, delivery: { state: "rejected", rejection: view.rejection } };
    case "stopped":
      return { ...base, delivery: view.failure === undefined ? { state: "stopped" } : { state: "stopped", failure: view.failure } };
    case "gone":
      return { ...base, delivery: { state: "gone", removed: view.removed } };
    case "idle":
    case "landed":
    case "discarded":
      return undefined;
  }
}
