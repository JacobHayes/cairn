// The sync chip's popover (design 9): pinned open by a click, or peeked by hover or keyboard
// focus. Two independent regions, so each can be left alone by the other: unpinning while the
// pointer is still on the chip leaves it peeked, and pinning does not end a peek. A peek ends a
// beat after the pointer or focus leaves, so the pointer can cross the gap into the popover;
// the beat is a delayed transition, and coming back during it keeps the popover. The document
// listeners (Esc, a press outside, a resize) and the focus moves stay in the chip, which sends
// `close`; where the popover opens is the dot's box when it opened, carried as plain data.
import { assign, setup, type SnapshotFrom } from "xstate";

/** How long a peek outlasts the pointer leaving, so the pointer can reach the popover. */
export const SYNC_POPOVER_LEAVE_MS = 150;

/** Where the popover opens: just below the dot, kept inside the window. */
export interface Anchor {
  top: number;
  left: number;
}

export type PopoverEvent =
  /** A click: pins it open, or unpins it. */
  | { type: "pin"; anchor: Anchor }
  /** Hover or focus arrived. */
  | { type: "peek"; anchor: Anchor }
  /** Hover or focus left. */
  | { type: "unpeek" }
  /** Esc, a press outside or a resize: both regions close at once. */
  | { type: "close" };

export const syncPopover = setup({
  types: { context: {} as { anchor: Anchor }, events: {} as PopoverEvent },
  actions: {
    place: assign({ anchor: ({ event }) => (event as Extract<PopoverEvent, { type: "pin" | "peek" }>).anchor }),
  },
  delays: { leave: SYNC_POPOVER_LEAVE_MS },
}).createMachine({
  id: "syncPopover",
  type: "parallel",
  context: { anchor: { top: 0, left: 0 } },
  states: {
    pin: {
      initial: "off",
      states: {
        off: { on: { pin: { target: "on", actions: "place" } } },
        on: { on: { pin: { target: "off", actions: "place" } } },
      },
    },
    peek: {
      initial: "off",
      states: {
        off: { on: { peek: { target: "on", actions: "place" } } },
        on: { on: { peek: { actions: "place" }, unpeek: "leaving" } },
        leaving: { after: { leave: "off" }, on: { peek: { target: "on", actions: "place" }, unpeek: { target: "leaving", reenter: true } } },
      },
    },
  },
  on: { close: { target: [".pin.off", ".peek.off"] } },
});

type PopoverSnapshot = SnapshotFrom<typeof syncPopover>;

/** Whether the popover is pinned open. */
export const popoverPinned = (snapshot: PopoverSnapshot): boolean => snapshot.matches({ pin: "on" });

/** Whether the popover is visible: pinned, or peeked (a leaving peek is still visible). */
export const popoverShown = (snapshot: PopoverSnapshot): boolean => popoverPinned(snapshot) || !snapshot.matches({ peek: "off" });

/** Where it opens. */
export const popoverAnchor = (snapshot: PopoverSnapshot): Anchor => snapshot.context.anchor;
