// D1, D4, B6, H5: what a node's actions have open in place of the action row. One machine per
// node, owned by the component, holds which form is open (never two), the draft in it, the
// revisions it was opened at, and the write it sent. The open case is saved as one draft, so a
// reload brings back exactly one form; a draft that fails validation is dropped.
//
//   closed -> open.editing -> open.sent -> closed          the sent version ended: nothing was typed since
//                                       -> open.kept       the version sent landed; text was typed since
//                                       -> open.editing    it was discarded, or its end was not seen
//
// Submitting is the control's: it gives the edit manager the write and tells this machine the
// write's id and the version of the draft it was built from. From then on the machine watches
// that write. Text typed after a submit is a newer version, so the write's end closes the form
// only when its version is still the one sent: a landing leaves the newer text in a read-only
// block (`kept`), and a write that was discarded or whose end was not seen leaves it editable,
// claiming no success. Closing a form never discards a write that is out or may have committed.
import { assign, fromCallback, setup, type EventObject, type SnapshotFrom } from "xstate";

import type { EvidenceDraft } from "../acting/acts.ts";
import { readDraft, writeDraft } from "../data/drafts.ts";
import type { OperationId } from "../data/edit-record.ts";
import type { EditsRef } from "../data/session-root.ts";
import { UNTIL_OPTIONS, type SnoozeDraft } from "./snooze.ts";
import type { Seen } from "./write.ts";

/** What each form holds. */
export interface FormValues {
  skip: string;
  keep: string;
  include: string;
  bypass: string;
  anyway: string;
  snooze: SnoozeDraft;
  evidence: EvidenceDraft;
  "edit-date": string;
}

export type FormCase = keyof FormValues;

/** Panels hold no draft of their own: their fields are local to them, or kept by their own editors. */
export type PanelCase = "assign" | "weight" | "break-down";

/** The write a form sent and the version of its draft the write was built from. */
export interface SentVersion {
  id: OperationId;
  version: number;
}

/** A form open on a draft. */
export type Drafted = {
  [K in FormCase]: {
    case: K;
    value: FormValues[K];
    /** The revisions the form was opened at: what its write is drafted against (H5, E6). */
    seen: Seen;
    /** Counted up on each change of `value`. */
    version: number;
    sent?: SentVersion;
    /** An earlier version landed: `value` is what was typed since, shown read-only. */
    kept?: true;
  };
}[FormCase];

/** What is open: a form on a draft, or a panel. */
export type Opened = Drafted | { case: PanelCase };

const FORM_CASES: readonly FormCase[] = ["skip", "keep", "include", "bypass", "anyway", "snooze", "evidence", "edit-date"];
const PANEL_CASES: readonly PanelCase[] = ["assign", "weight", "break-down"];

/** A form opened on `value`, as drafted against `seen`. */
export function opening<K extends FormCase>(kind: K, value: FormValues[K], seen: Seen): Drafted {
  return { case: kind, value, seen: { base: seen.base, deployment: seen.deployment }, version: 0 } as Drafted;
}

/** Whether `opened` is a form on a draft rather than a panel. */
export function isDrafted(opened: Opened | undefined): opened is Drafted {
  return opened !== undefined && "version" in opened;
}

/** The draft key of node `node` of journey `journey`: the one draft all its actions share. */
export function actionsDraftKey(journey: string, node: string): string {
  return `actions:${journey}:${node}`;
}

type Plain = Record<string, unknown>;

const isPlain = (value: unknown): value is Plain => typeof value === "object" && value !== null && !Array.isArray(value);
const isText = (value: unknown): value is string => typeof value === "string";
const isCount = (value: unknown): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0;

const isSnoozeDraft = (value: unknown): value is SnoozeDraft =>
  isPlain(value) && isText(value.aside) && isText(value.date) && isText(value.target) && UNTIL_OPTIONS.some((option) => option.value === value.until);

const isEvidenceDraft = (value: unknown): value is EvidenceDraft => isPlain(value) && isText(value.artifact) && isText(value.note);

const valueReaders: { [K in FormCase]: (value: unknown) => value is FormValues[K] } = {
  skip: isText,
  keep: isText,
  include: isText,
  bypass: isText,
  anyway: isText,
  snooze: isSnoozeDraft,
  evidence: isEvidenceDraft,
  "edit-date": isText,
};

function readSent(value: unknown, version: number): SentVersion | undefined {
  return isPlain(value) && isText(value.id) && isCount(value.version) && value.version <= version ? { id: value.id, version: value.version } : undefined;
}

/**
 * The open case `value` is, or none when it is not one this build can show: an unknown case, a
 * value of the wrong shape for its case, revisions or versions that are not counts, a write
 * claimed to be sent at a version the draft has not reached, or a kept flag that is not true.
 * What it answers holds only the fields it knows.
 */
export function readOpened(value: unknown): Opened | undefined {
  if (!isPlain(value)) {
    return undefined;
  }
  const kind = value.case;
  if (PANEL_CASES.some((each) => each === kind)) {
    return { case: kind as PanelCase };
  }
  const form = FORM_CASES.find((each) => each === kind);
  if (form === undefined || !valueReaders[form](value.value) || !isPlain(value.seen) || !isCount(value.seen.base) || !isCount(value.seen.deployment) || !isCount(value.version)) {
    return undefined;
  }
  const sent = value.sent === undefined ? undefined : readSent(value.sent, value.version);
  if ((value.sent !== undefined && sent === undefined) || (value.kept !== undefined && value.kept !== true) || (sent !== undefined && value.kept === true)) {
    return undefined;
  }
  return {
    case: form,
    value: value.value,
    seen: { base: value.seen.base, deployment: value.seen.deployment },
    version: value.version,
    ...(sent === undefined ? {} : { sent }),
    ...(value.kept === true ? { kept: true } : {}),
  } as Drafted;
}

/** The draft kept under `key`, taken for use; one that fails validation is removed, and nothing opens. */
export function takeOpened(key: string): Opened | undefined {
  const opened = readOpened(readDraft<unknown>(key));
  if (opened === undefined) {
    writeDraft(key, undefined);
  }
  return opened;
}

/** How a watched write ended: it landed, was discarded, or was no longer held when the watch began (its end was not seen). */
export type Outcome = "landed" | "discarded" | "unknown";

export interface ActionsFormInput {
  edits: EditsRef;
  /** The draft kept from before, already validated. */
  restored: Opened | undefined;
  /** Keeps `opened` as the node's one draft, or forgets the draft when none is open. */
  save: (opened: Opened | undefined) => void;
}

interface ActionsFormContext extends ActionsFormInput {
  opened: Opened | undefined;
}

export type ActionsFormEvent =
  | { type: "open"; opened: Opened }
  | { [K in FormCase]: { type: "change"; case: K; value: FormValues[K] } }[FormCase]
  /** The form submitted the write `id`, built from the draft at `version`. */
  | { type: "sent"; id: OperationId; version: number }
  /** A retry sent a new write in the place of the sent write `from`: the same draft version. */
  | { type: "resent"; from: OperationId; id: OperationId }
  /** The watched write ended. */
  | { type: "settled"; id: OperationId; outcome: Outcome }
  | { type: "close" };

/** The event that changes the open `kind` form's draft to `value`. */
export function changing<K extends FormCase>(kind: K, value: FormValues[K]): ActionsFormEvent {
  return { type: "change", case: kind, value } as ActionsFormEvent;
}

/**
 * Reports when the write `id` ends. A write the manager does not hold ended while the form was
 * away (a reload, another screen), or its record was dropped on restore: its end is `unknown`.
 * Only a write's own end counts: a write the manager replaces by a resubmit is stopped, which
 * is not an end, and the control tells the form with `resent`.
 */
const watch = fromCallback<EventObject, { edits: EditsRef; id: OperationId }>(({ input, sendBack }) => {
  const report = (outcome: Outcome) => {
    sendBack({ type: "settled", id: input.id, outcome });
  };
  const entry = input.edits.getSnapshot().context.writes[input.id];
  if (entry === undefined) {
    report("unknown");
    return undefined;
  }
  const ended = (snapshot: SnapshotFrom<typeof entry.ref>): boolean => {
    if (snapshot.status !== "done") {
      return false;
    }
    report(snapshot.matches("landed") ? "landed" : "discarded");
    return true;
  };
  if (ended(entry.ref.getSnapshot())) {
    return undefined;
  }
  const subscription = entry.ref.subscribe((snapshot) => {
    ended(snapshot);
  });
  return () => {
    subscription.unsubscribe();
  };
});

const draftedOf = (context: ActionsFormContext): Drafted | undefined => (isDrafted(context.opened) ? context.opened : undefined);

/** `opened` without the write it sent. */
function unsent(opened: Drafted): Drafted {
  return Object.fromEntries(Object.entries(opened).filter(([field]) => field !== "sent")) as Drafted;
}

export const actionsForm = setup({
  types: {
    context: {} as ActionsFormContext,
    events: {} as ActionsFormEvent,
    input: {} as ActionsFormInput,
  },
  actors: { watch },
  guards: {
    restoredSent: ({ context }) => draftedOf(context)?.sent !== undefined,
    restoredKept: ({ context }) => draftedOf(context)?.kept === true,
    restoredOpen: ({ context }) => context.opened !== undefined,
    // Opening the case already open keeps what was typed, as long as it is still a form to type in.
    alreadyOpen: ({ context, event }) => event.type === "open" && context.opened?.case === event.opened.case && !(draftedOf(context)?.kept === true),
    changesOpen: ({ context, event }) => event.type === "change" && draftedOf(context)?.case === event.case,
    ownSettled: ({ context, event }) => event.type === "settled" && draftedOf(context)?.sent?.id === event.id,
    // Nothing was typed since the version that was sent.
    unchangedSettled: ({ context, event }) => {
      const drafted = draftedOf(context);
      return event.type === "settled" && drafted?.sent?.id === event.id && drafted.sent.version === drafted.version;
    },
    ownLanded: ({ context, event }) => event.type === "settled" && event.outcome === "landed" && draftedOf(context)?.sent?.id === event.id,
    drafting: ({ context }) => draftedOf(context) !== undefined,
    resentFrom: ({ context, event }) => event.type === "resent" && draftedOf(context)?.sent?.id === event.from,
  },
  actions: {
    take: assign({ opened: ({ event }) => (event.type === "open" ? event.opened : undefined) }),
    edit: assign({
      opened: ({ context, event }) => {
        const drafted = draftedOf(context);
        return drafted === undefined || event.type !== "change" ? context.opened : ({ ...drafted, value: event.value, version: drafted.version + 1 } as Drafted);
      },
    }),
    send: assign({
      opened: ({ context, event }) => {
        const drafted = draftedOf(context);
        return drafted === undefined || event.type !== "sent" ? context.opened : { ...drafted, sent: { id: event.id, version: event.version } };
      },
    }),
    resend: assign({
      opened: ({ context, event }) => {
        const drafted = draftedOf(context);
        return drafted?.sent === undefined || event.type !== "resent" ? context.opened : { ...drafted, sent: { id: event.id, version: drafted.sent.version } };
      },
    }),
    unsend: assign({
      opened: ({ context }) => {
        const drafted = draftedOf(context);
        return drafted === undefined ? context.opened : unsent(drafted);
      },
    }),
    keep: assign({
      opened: ({ context }) => {
        const drafted = draftedOf(context);
        return drafted === undefined ? context.opened : { ...unsent(drafted), kept: true };
      },
    }),
    clear: assign({ opened: undefined }),
    // Closing drops a write only when it is settled: rejected, stopped or gone. One that is out or may have committed stays.
    discardSettled: ({ context }) => {
      const sent = draftedOf(context)?.sent;
      const entry = sent === undefined ? undefined : context.edits.getSnapshot().context.writes[sent.id];
      if (sent !== undefined && entry?.ref.getSnapshot().hasTag("settled") === true) {
        context.edits.send({ type: "discard", id: sent.id });
      }
    },
    // Every change of what is open is kept; exit actions are not (they do not run when the actor stops).
    persist: ({ context }) => {
      context.save(context.opened);
    },
  },
}).createMachine({
  id: "actionsForm",
  context: ({ input }) => ({ ...input, opened: input.restored }),
  initial: "start",
  states: {
    start: {
      always: [
        { guard: "restoredSent", target: "open.sent" },
        { guard: "restoredKept", target: "open.kept" },
        { guard: "restoredOpen", target: "open.editing" },
        { target: "closed" },
      ],
    },
    closed: { on: { open: { target: "open.editing", actions: ["take", "persist"] } } },
    open: {
      on: {
        open: [{ guard: "alreadyOpen" }, { target: ".editing", reenter: true, actions: ["take", "persist"] }],
        close: { target: "closed", actions: ["discardSettled", "clear", "persist"] },
      },
      initial: "editing",
      states: {
        editing: {
          on: {
            change: { guard: "changesOpen", actions: ["edit", "persist"] },
            sent: { guard: "drafting", target: "sent", actions: ["send", "persist"] },
          },
        },
        sent: {
          invoke: { src: "watch", input: ({ context }) => ({ edits: context.edits, id: draftedOf(context)?.sent?.id ?? "" }) },
          on: {
            change: { guard: "changesOpen", actions: ["edit", "persist"] },
            // A resubmit after a rejection: the previous write is the manager's to drop.
            sent: { guard: "drafting", target: "sent", reenter: true, actions: ["send", "persist"] },
            resent: { guard: "resentFrom", target: "sent", reenter: true, actions: ["resend", "persist"] },
            settled: [
              { guard: "unchangedSettled", target: "#actionsForm.closed", actions: ["clear", "persist"] },
              { guard: "ownLanded", target: "kept", actions: ["keep", "persist"] },
              { guard: "ownSettled", target: "editing", actions: ["unsend", "persist"] },
            ],
          },
        },
        // An earlier version landed: what was typed since is shown, read-only, until it is closed.
        kept: {},
      },
    },
  },
});

export type ActionsFormSnapshot = SnapshotFrom<typeof actionsForm>;

/** What the component draws of the form; tagged, so none pieces it together from optional fields. */
export type FormView =
  | { state: "closed" }
  | { state: "editing"; opened: Opened }
  | { state: "sending"; opened: Drafted; sent: SentVersion }
  | { state: "kept"; opened: Drafted };

export function formView(snapshot: ActionsFormSnapshot): FormView {
  const { opened } = snapshot.context;
  const drafted = isDrafted(opened) ? opened : undefined;
  if (opened === undefined || snapshot.matches("closed")) {
    return { state: "closed" };
  }
  if (snapshot.matches({ open: "kept" }) && drafted !== undefined) {
    return { state: "kept", opened: drafted };
  }
  if (snapshot.matches({ open: "sent" }) && drafted?.sent !== undefined) {
    return { state: "sending", opened: drafted, sent: drafted.sent };
  }
  return { state: "editing", opened };
}
