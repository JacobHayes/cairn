// The toast: a confirmation that is not about syncing ("Route file exported"),
// shown briefly at the workspace's bottom right and replaced by the next. Its lifetime is a
// machine: it counts down while shown (a delayed transition), the count pauses while the
// pointer is on it and restarts in full when the pointer leaves, and a new toast replaces the
// one on screen and takes its own full count. The session root runs it; the shell shows it.
import { assign, setup, type ActorRefFrom, type SnapshotFrom } from "xstate";

/** How long a toast stays; a problem stays longer, since it says what to do next. */
export const TOAST_MS = { confirm: 2500, problem: 6000 };

export interface Toast {
  /** A new one for each toast, so a replacement is a new thing on screen. */
  id: number;
  tone: "confirm" | "problem";
  text: string;
}

interface ToastContext {
  toast: Toast | undefined;
  /** The toasts shown so far: the next id. */
  count: number;
}

export type ToastEvent =
  /** Shows `text` as the toast, replacing the one on screen. */
  | { type: "show"; text: string; tone: Toast["tone"] }
  /** The pointer is on the toast: it stays. */
  | { type: "hover" }
  /** The pointer left it: it goes after its full time. */
  | { type: "leave" };

export const toastMachine = setup({
  types: { context: {} as ToastContext, events: {} as ToastEvent },
  actions: {
    put: assign(({ context, event }) => {
      const shown = event as Extract<ToastEvent, { type: "show" }>;
      return { toast: { id: context.count + 1, tone: shown.tone, text: shown.text }, count: context.count + 1 };
    }),
    clear: assign({ toast: undefined }),
  },
  delays: {
    expiry: ({ context }) => (context.toast === undefined ? 0 : TOAST_MS[context.toast.tone]),
  },
}).createMachine({
  id: "toast",
  context: { toast: undefined, count: 0 },
  initial: "hidden",
  states: {
    hidden: { entry: "clear", on: { show: { target: "shown", actions: "put" } } },
    shown: {
      initial: "counting",
      states: {
        // A toast shown while another counts down replaces it and takes a full count of its own.
        counting: { after: { expiry: "#toast.hidden" }, on: { hover: "held", show: { target: "counting", reenter: true, actions: "put" } } },
        // A toast shown while the pointer holds another takes its place and is held too.
        held: { on: { leave: "counting", show: { actions: "put" } } },
      },
    },
  },
});

export type ToastsRef = ActorRefFrom<typeof toastMachine>;

/** The toast on screen, if any. */
export const toastOf = (snapshot: SnapshotFrom<typeof toastMachine>): Toast | undefined => snapshot.context.toast;
