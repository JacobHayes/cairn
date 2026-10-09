// The app frame's slots (design: the grid frame). The shell owns three regions beside the
// workspace: the inspector column (a bottom sheet on a tablet, stacked on a phone), its
// assistant tab, and the strip. A screen puts its node detail in the inspector with
// `<Inspector>` and the assistant puts its panel in the assistant tab with `<AssistantSlot>`:
// each renders into the shell's slot through a portal, so the screens keep their own state
// and the shell only learns that something is there (it shows the column when so).
//
// Outside the shell (a component rendered alone in a test) there is no slot, and both render
// their children where they stand.
import { createContext, useContext, useEffect, useLayoutEffect, useSyncExternalStore, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useLocation } from "react-router";

import { screenPath } from "../detail/parts.tsx";

export type InspectorTab = "inspector" | "assistant";

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
  tab: InspectorTab;
}

/** The assistant's toggle, which the tab bar uses to open it from the shell. */
export interface AssistantDock {
  open: () => void;
  close: () => void;
}

/** What a screen tells the shell. Stable for the shell's life, so effects keyed on it run once. */
export interface FrameActions {
  /** Something is in `slot`'s pane until the returned function is called. */
  mount: (slot: InspectorTab) => () => void;
  /** Brings `tab` into view. */
  show: (tab: InspectorTab) => void;
  /** The assistant can be opened (its toggle is on screen) until the returned function is called. */
  dock: (dock: AssistantDock) => () => void;
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

/**
 * The inspector's content, rendered in the shell's inspector column. `focus` names what it is
 * about (the open node): when it changes, the inspector tab comes into view. Closing sends
 * the sheet's Esc to the screen the node is open on, keeping what that screen shows. With
 * `reveal` off (the journey card, which is there without being asked for) it leaves the tab
 * in view alone and a phone does not scroll to it.
 */
export function Inspector({ focus, reveal = true, children }: { focus: string; reveal?: boolean; children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
  const { pathname, search } = useLocation();
  const close = `${screenPath(pathname)}${search}`;
  // Layout effects, so the column appears in the same commit as the screen that fills it: a second
  // render after paint could land in a ResizeObserver delivery and make the canvas resize twice.
  useLayoutEffect(() => actions?.mount("inspector"), [actions]);
  useLayoutEffect(() => {
    if (reveal) {
      actions?.show("inspector");
    }
  }, [actions, focus, reveal]);
  // On a phone the inspector stacks under the whole page: bring it into view, or opening a node looks like nothing happened.
  const mapOpen = new URLSearchParams(search).get("map") === "1";
  const column = state?.inspector?.parentElement;
  useEffect(() => {
    if (!reveal || column === null || column === undefined || mapOpen || !globalThis.matchMedia(PHONE_WIDTH).matches) {
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

/** Registers the assistant's opener with the shell while the toggle is on screen. */
export function useAssistantDock(dock: AssistantDock | undefined): void {
  const actions = useFrameActions();
  useEffect(() => {
    if (actions === undefined || dock === undefined) {
      return undefined;
    }
    return actions.dock(dock);
  }, [actions, dock]);
}
