// The app frame's slots (design: the grid frame). The shell owns three regions beside the
// workspace: the inspector column (a bottom sheet on a tablet, stacked on a phone) and its
// assistant tab. A screen puts its node detail in the inspector with
// `<Inspector>` and the assistant puts its panel in the assistant tab with `<AssistantSlot>`:
// each renders into the shell's slot through a portal, so the screens keep their own state
// and the shell only learns that something is there (it shows the column when so).
//
// Outside the shell (a component rendered alone in a test) there is no slot, and both render
// their children where they stand.
import { createContext, useContext, useEffect, useLayoutEffect, useSyncExternalStore, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useLocation } from "react-router";

import { useNodeAddresses } from "../journeys/address.ts";

export type InspectorTab = "inspector" | "assistant" | "proposals";

/** The window widths that make the inspector a bottom sheet (design 3.3). */
export const SHEET = "(min-width: 720px) and (max-width: 1099px)";

/** The widths that give the inspector a column of its own (design 3.3); narrower, it is a sheet or stacked. */
export const COLUMN = "(min-width: 1100px)";
/** A phone's width: the inspector stacks under the page. */
export const PHONE_WIDTH = "(max-width: 719px)";

/** Whether the window matches `query` now, and as it changes. */
export function useMedia(query: string): boolean {
  return useSyncExternalStore(
    (notify) => {
      const list = globalThis.matchMedia(query);
      list.addEventListener("change", notify);
      return () => {
        list.removeEventListener("change", notify);
      };
    },
    () => globalThis.matchMedia(query).matches,
  );
}

/** What the shell shows of itself: the slot elements, once mounted, and the tab in view. */
export interface FrameState {
  inspector: HTMLElement | null;
  assistant: HTMLElement | null;
  proposals: HTMLElement | null;
  /** How many open proposals the Proposals tab holds, once read; undefined while reading or failed. */
  proposalsCount: number | undefined;
  tab: InspectorTab;
  /** Opens the assistant panel; undefined where none is offered. */
  openAssistant: (() => void) | undefined;
}

/** The assistant's controls, which the inspector column's tabs and Esc use from the shell. */
export interface AssistantDock {
  open: () => void;
  close: () => void;
}

/** What a screen tells the shell. Stable for the shell's life, so effects keyed on it run once. */
export interface FrameActions {
  /** Something is in `slot`'s pane until the returned function is called. */
  mount: (slot: InspectorTab) => () => void;
  /** Brings `tab` into view whatever the viewer left in its way: the other tab, a hidden column. */
  show: (tab: InspectorTab) => void;
  /** The assistant can be opened (its tab is offered) until the returned function is called. */
  dock: (dock: AssistantDock) => () => void;
  /** The Proposals tab's count, until the returned function is called. */
  countProposals: (count: number | undefined) => () => void;
  /** Where Esc takes the sheet: `address` closes the open node; undefined when nothing is open. */
  closeTo: (address: string | undefined) => void;
}

export const FrameStateContext = createContext<FrameState | undefined>(undefined);
export const FrameActionsContext = createContext<FrameActions | undefined>(undefined);

/** The shell's frame state, or undefined outside it. */
export function useFrameState(): FrameState | undefined {
  return useContext(FrameStateContext);
}

/** The shell's frame actions, or undefined outside it. */
export function useFrameActions(): FrameActions | undefined {
  return useContext(FrameActionsContext);
}

/** On a phone, where the column is stacked under the page, scrolls it into view when `focus` changes and `reveal` holds. */
function useScrollToColumn(column: HTMLElement | null | undefined, focus: string, reveal: boolean): void {
  useEffect(() => {
    if (!reveal || column === null || column === undefined || !globalThis.matchMedia(PHONE_WIDTH).matches) {
      return undefined;
    }
    // After the commit that shows the column, which is hidden until a screen fills it.
    const frame = requestAnimationFrame(() => {
      const calm = globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches;
      column.scrollIntoView({ block: "start", behavior: calm ? "auto" : "smooth" });
    });
    return () => {
      cancelAnimationFrame(frame);
    };
    // Picking another node brings it into view; the map opening or closing does not.
  }, [focus, column, reveal]);
}

/**
 * The inspector's content, rendered in the shell's inspector column. `focus` names what it is
 * about (the open node): when it changes, the inspector tab comes into view, even if the viewer hid the column. Closing sends
 * the sheet's Esc to the screen the node is open on, keeping what that screen shows. With
 * `reveal` off (the journey card, which is there without being asked for) it leaves the tab
 * in view alone and a phone does not scroll to it.
 */
export function Inspector({ focus, reveal = true, children }: { focus: string; reveal?: boolean; children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const close = addresses.close();
  // Layout effects, so the column appears in the same commit as the screen that fills it: a second
  // render after paint could land in a ResizeObserver delivery and make the canvas resize twice.
  useLayoutEffect(() => actions?.mount("inspector"), [actions]);
  useLayoutEffect(() => {
    if (reveal) {
      actions?.show("inspector");
    }
  }, [actions, focus, reveal]);
  // The column is one scroller shared by every selection: a node opens at its header and its sentence, not part-way down the last one's body.
  const body = state?.inspector;
  useLayoutEffect(() => {
    if (body !== null && body !== undefined) {
      body.scrollTop = 0;
    }
  }, [body, focus]);
  // On a phone the inspector stacks under the whole page: bring it into view, or opening a node looks like nothing happened.
  const mapOpen = new URLSearchParams(search).get("map") === "1";
  useScrollToColumn(state?.inspector?.parentElement, focus, reveal && !mapOpen);
  useEffect(() => {
    actions?.closeTo(close);
    return () => {
      actions?.closeTo(undefined);
    };
  }, [actions, close]);
  if (actions === undefined || state === undefined) {
    return children;
  }
  return state.inspector === null ? null : createPortal(children, state.inspector);
}

/**
 * The destination's open proposals, rendered in the inspector column's Proposals tab (C14). The
 * tab is there for as long as the page is, at zero too; `count` shows beside its name. With
 * `reveal` (the page was opened at its `/proposals` address) the tab comes into view.
 */
export function ProposalsSlot({ count, reveal, children }: { count: number | undefined; reveal: boolean; children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
  useLayoutEffect(() => actions?.mount("proposals"), [actions]);
  useLayoutEffect(() => actions?.countProposals(count), [actions, count]);
  const { key } = useLocation();
  useLayoutEffect(() => {
    if (reveal) {
      actions?.show("proposals");
    }
  }, [actions, reveal, key]);
  useScrollToColumn(state?.proposals?.parentElement, "proposals", reveal);
  if (actions === undefined || state === undefined) {
    return children;
  }
  return state.proposals === null ? null : createPortal(children, state.proposals);
}

/** The assistant's panel, rendered in the inspector column's assistant tab and brought into view. */
export function AssistantSlot({ children }: { children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
  useLayoutEffect(() => {
    const unmount = actions?.mount("assistant");
    actions?.show("assistant");
    return unmount;
  }, [actions]);
  if (actions === undefined || state === undefined) {
    return children;
  }
  return state.assistant === null ? null : createPortal(children, state.assistant);
}

/** Registers the assistant's opener with the shell while it is offered. */
export function useAssistantDock(dock: AssistantDock | undefined): void {
  const actions = useFrameActions();
  useEffect(() => {
    if (actions === undefined || dock === undefined) {
      return undefined;
    }
    return actions.dock(dock);
  }, [actions, dock]);
}
