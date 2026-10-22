// The app frame's slots (design: the grid frame). The shell owns three regions beside the
// workspace: the inspector column (a bottom sheet on a tablet and a phone) and its
// assistant tab. A screen puts its node detail in the inspector with
// `<Inspector>` and the assistant puts its panel in the assistant tab with `<AssistantSlot>`:
// each renders into the shell's slot through a portal, so the screens keep their own state
// and the shell only learns that something is there (it shows the column when so).
//
// Outside the shell (a component rendered alone in a test) there is no slot, and both render
// their children where they stand.
import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useSyncExternalStore, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useLocation } from "react-router";

import { useNodeAddresses } from "../journeys/address.ts";
import type { KeyBinding } from "../screens/keys.ts";
import type { SheetSnap } from "./InspectorColumn.tsx";

export type InspectorTab = "inspector" | "assistant" | "proposals";

/** The window widths that make the inspector a bottom sheet, tablet and phone alike (design 3.3). */
export const SHEET = "(max-width: 1099px)";

/** The widths that give the inspector a column of its own (design 3.3); narrower, it is a sheet. */
export const COLUMN = "(min-width: 1100px)";
/** A phone's width: the top bar replaces the rail and the sheet opens at its peek. */
export const PHONE_WIDTH = "(max-width: 719px)";

/** Windows where the sheet opens at its peek: a phone's width, or any window too short to give up half its height to it. */
export const PEEK_FIRST = `${PHONE_WIDTH}, (max-height: 599px)`;

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

/** The keys a page binds and the way to open its key sheet: what the rail's `?` previews and opens. */
export interface PageKeys {
  bindings: readonly KeyBinding[];
  open: () => void;
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
  /** The keys of the page on screen; undefined where the page binds none (the rail draws no `?`). */
  keys: PageKeys | undefined;
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
  /** The page's keys, until the returned function is called. */
  bindKeys: (keys: PageKeys) => () => void;
  /** The Proposals tab's count, until the returned function is called. */
  countProposals: (count: number | undefined) => () => void;
  /** Where Esc takes the sheet: `address` closes the open node; undefined when nothing is open. */
  closeTo: (address: string | undefined) => void;
  /** Sets the sheet's height (peek, half, full); a column has no height to set. */
  snap: (to: SheetSnap) => void;
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
 * about (the open node): when it changes, the inspector tab comes into view, even if the viewer hid the column. Closing sends
 * the sheet's Esc to the screen the node is open on, keeping what that screen shows. With
 * `reveal` off (the journey card, which is there without being asked for) it leaves the tab
 * in view alone.
 */
export function Inspector({ focus, reveal = true, children }: { focus: string; reveal?: boolean; children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
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
  // The journey card (not revealed) is no open node: Esc has nothing to go back from, and closes the sheet.
  useEffect(() => {
    actions?.closeTo(reveal ? close : undefined);
    return () => {
      actions?.closeTo(undefined);
    };
  }, [actions, close, reveal]);
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
  if (actions === undefined || state === undefined) {
    return children;
  }
  return state.proposals === null ? null : createPortal(children, state.proposals);
}

/**
 * The assistant's panel, rendered in the inspector column's assistant tab. Mounting it chooses no
 * tab: a remount (a reload, a resize that rebuilds the page) leaves the tab in view alone, and
 * only a person's click (the tab, or a screen's button through `openAssistant`) brings it forward.
 */
export function AssistantSlot({ children }: { children: ReactNode }) {
  const actions = useFrameActions();
  const state = useFrameState();
  useLayoutEffect(() => actions?.mount("assistant"), [actions]);
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

/** Registers the page's keys with the shell while it is on screen: the rail's `?` previews them and opens the sheet (`setSheetOpen(true)`). `bindings` should be stable. */
export function usePageKeys(bindings: readonly KeyBinding[], setSheetOpen: (open: boolean) => void): void {
  const actions = useFrameActions();
  const keys = useMemo(() => ({ bindings, open: () => { setSheetOpen(true); } }), [bindings, setSheetOpen]);
  useEffect(() => actions?.bindKeys(keys), [actions, keys]);
}
