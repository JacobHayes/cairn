// The tooltip's timing, as a machine the one TipLayer owns (xstate, as the sync popover's peek
// is: the timed parts are actors). A tip shows once the pointer has rested on its thing for
// `TIP_SHOW_MS`, at once while another has just shown (within `TIP_SKIP_MS` of it going), and at
// once on keyboard focus or a tap. A tip opened by hover is the pointer's: it goes a beat
// (`HOVER_LEAVE_MS`) after the pointer leaves, and coming back in the beat (to the thing or to the
// tip) keeps it. A tip opened by focus or a tap stays until blur, a dismiss or a second tap,
// whatever the pointer does. A dismissed thing stays quiet until the pointer has left it and come
// back. Everything the layer needs to draw a tip is plain data on the target.
import type { ReactNode } from "react";
import { assign, setup, type SnapshotFrom } from "xstate";

/** How long the pointer rests on a thing before its tip shows (ms). */
export const TIP_SHOW_MS = 300;
/** How long after a tip goes the next one still shows at once (ms). */
export const TIP_SKIP_MS = 500;
/** How long a hover tip outlasts the pointer leaving, so the pointer can cross onto it (ms): the sync popover's peek waits the same beat. */
export const HOVER_LEAVE_MS = 150;

/**
 * What a tip is about. `id` is its identity (the trigger's `data-tip`, or a point tip's line). An
 * element target is placed against the element's box; a point target against the pointer or box
 * where it opened, and carries the `content` the layer draws.
 */
export interface TipTarget {
  id: string;
  element?: HTMLElement;
  /** A point target: where it opened, as an anchor box. */
  box?: { left: number; top: number; width: number; height: number };
  content?: ReactNode;
}

export type TipVia = "pointer" | "focus" | "tap";

export type TipEvent =
  /** The pointer is over a thing (every move over it too: a move restarts the rest). */
  | { type: "hover"; target: TipTarget }
  /** The pointer left a thing and its tip. */
  | { type: "unhover" }
  /** Keyboard focus reached a thing. */
  | { type: "focus"; target: TipTarget }
  | { type: "blur" }
  /** A tap or click on a tap thing, or Enter or Space on it. */
  | { type: "tap"; target: TipTarget }
  /** Esc, a press, a scroll, a resize: closes at once and keeps the thing quiet until the pointer comes back. */
  | { type: "dismiss" }
  /** The thing that owns a point target is gone (its view unmounted): its tip goes with it, and the thing is not kept quiet. */
  | { type: "drop"; id: string };

interface Context {
  target: TipTarget | undefined;
  via: TipVia;
  /** The thing a dismiss closed: it does not reopen until the pointer has left it. */
  quiet: string | undefined;
}

export const tipMachine = setup({
  types: { context: {} as Context, events: {} as TipEvent },
  actions: {
    pointer: assign({ target: ({ event }) => (event as Extract<TipEvent, { type: "hover" }>).target, via: "pointer" as TipVia, quiet: undefined }),
    keyboard: assign({ target: ({ event }) => (event as Extract<TipEvent, { type: "focus" }>).target, via: "focus" as TipVia }),
    tap: assign({ target: ({ event }) => (event as Extract<TipEvent, { type: "tap" }>).target, via: "tap" as TipVia }),
    quiet: assign({ quiet: ({ context }) => context.target?.id }),
    forgetQuiet: assign({ quiet: undefined }),
    clear: assign({ target: undefined }),
  },
  guards: {
    isQuiet: ({ context, event }) => event.type === "hover" && event.target.id === context.quiet,
    isTarget: ({ context, event }) => event.type === "drop" && event.id === context.target?.id,
    viaPointer: ({ context }) => context.via === "pointer",
    viaFocus: ({ context }) => context.via === "focus",
    toggledOff: ({ context, event }) => event.type === "tap" && event.target.id === context.target?.id && context.via === "tap",
  },
  delays: { show: TIP_SHOW_MS, skip: TIP_SKIP_MS, leave: HOVER_LEAVE_MS },
}).createMachine({
  id: "tip",
  initial: "idle",
  on: { drop: { guard: "isTarget", target: ".idle", actions: "clear" } },
  context: { target: undefined, via: "pointer", quiet: undefined },
  states: {
    idle: {
      on: {
        hover: [{ guard: "isQuiet" }, { target: "waiting", actions: "pointer" }],
        unhover: { actions: "forgetQuiet" },
        focus: { target: "open", actions: "keyboard" },
        tap: { target: "open", actions: "tap" },
      },
    },
    waiting: {
      after: { show: "open" },
      on: {
        // A move restarts the rest, and the tip opens where the pointer rested.
        hover: { target: "waiting", reenter: true, actions: "pointer" },
        unhover: { target: "idle", actions: "clear" },
        focus: { target: "open", actions: "keyboard" },
        tap: { target: "open", actions: "tap" },
        dismiss: { target: "idle", actions: "quiet" },
      },
    },
    open: {
      on: {
        hover: [
          // A tip opened by focus or a tap is not the pointer's: hovering elsewhere neither replaces it nor closes it.
          { guard: "viaPointer", target: "open", actions: "pointer" },
        ],
        unhover: { guard: "viaPointer", target: "leaving" },
        focus: { target: "open", actions: "keyboard" },
        blur: { guard: "viaFocus", target: "warm", actions: "clear" },
        tap: [{ guard: "toggledOff", target: "idle", actions: "quiet" }, { target: "open", actions: "tap" }],
        dismiss: { target: "idle", actions: "quiet" },
      },
    },
    leaving: {
      after: { leave: { target: "warm", actions: "clear" } },
      on: {
        hover: { target: "open", actions: "pointer" },
        focus: { target: "open", actions: "keyboard" },
        tap: { target: "open", actions: "tap" },
        dismiss: { target: "idle", actions: "quiet" },
      },
    },
    warm: {
      after: { skip: "idle" },
      on: {
        hover: [{ guard: "isQuiet" }, { target: "open", actions: "pointer" }],
        unhover: { actions: "forgetQuiet" },
        focus: { target: "open", actions: "keyboard" },
        tap: { target: "open", actions: "tap" },
      },
    },
  },
});

export type TipSnapshot = SnapshotFrom<typeof tipMachine>;

/** Whether a tip is drawn: open, or in the beat after the pointer left. */
export const tipShown = (snapshot: TipSnapshot): boolean => snapshot.matches("open") || snapshot.matches("leaving");

/** What the shown tip is about, and how it was opened. */
export const tipTarget = (snapshot: TipSnapshot): { target: TipTarget; via: TipVia } | undefined =>
  tipShown(snapshot) && snapshot.context.target !== undefined ? { target: snapshot.context.target, via: snapshot.context.via } : undefined;
