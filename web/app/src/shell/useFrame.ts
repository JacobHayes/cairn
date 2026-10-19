// The frame's state: which of the inspector column's panes have something in them, which tab
// is in view, whether the viewer hid the column (`]`), and where Esc takes the sheet.
// The screens report through `FrameActions` (frame.tsx); the shell reads the result.
import { useCallback, useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { useNavigate } from "react-router";

import type { SheetSnap } from "./InspectorColumn.tsx";
import { typing } from "../ui/typing.ts";
import { SHEET, type AssistantDock, type FrameActions, type FrameState, type InspectorTab } from "./frame.tsx";

export interface Frame {
  state: FrameState;
  actions: FrameActions;
  /** The column is on screen: something is in a pane and the viewer has not hidden it. */
  open: boolean;
  /** Something is in a pane, or the assistant is offered, hidden or not. */
  present: boolean;
  /** The viewer hid it: the rail, or the sheet's bar, is all that shows. */
  folded: boolean;
  toggleFolded: () => void;
  snap: SheetSnap;
  setSnap: (snap: SheetSnap) => void;
  dock: AssistantDock | undefined;
  /** The tab bar's choice: the assistant's tab opens the assistant when it is closed. */
  choose: (tab: InspectorTab) => void;
  setInspector: (element: HTMLElement | null) => void;
  setAssistant: (element: HTMLElement | null) => void;
  setProposals: (element: HTMLElement | null) => void;
  /** The tabs on offer, in order. */
  tabs: InspectorTab[];
}

/** The assistant's opener for screens: opens it, and brings it into view when already open but hidden or behind the other tab. */
function useOpenAssistant(dock: AssistantDock | undefined, actions: FrameActions): (() => void) | undefined {
  const open = dock?.open;
  return useMemo(
    () =>
      open === undefined
        ? undefined
        : () => {
            open();
            actions.show("assistant");
          },
    [open, actions],
  );
}

/**
 * `]` hides and shows the inspector; Esc closes the tablet's sheet, unless a field has focus:
 * the assistant when its tab is in view, else the open node by going back to the screen it is
 * open on (`closeTo`).
 */
function useFrameKeys({ tab, dock, closeTo }: { tab: InspectorTab; dock: AssistantDock | undefined; closeTo: string | undefined }, fold: () => void): void {
  const navigate = useNavigate();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target)) {
        return;
      }
      if (event.key === "]") {
        fold();
      } else if (event.key === "Escape" && globalThis.matchMedia(SHEET).matches) {
        if (tab === "assistant") {
          dock?.close();
        } else if (closeTo !== undefined) {
          void navigate(closeTo);
        }
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  }, [tab, dock, closeTo, navigate, fold]);
}

/** How many screens have something in each pane. */
interface Counts {
  inspector: number;
  assistant: number;
  proposals: number;
}

/** The setters the frame's actions write through. */
interface Setters {
  setCounts: Dispatch<SetStateAction<Counts>>;
  setTab: (tab: InspectorTab) => void;
  setHidden: (hidden: boolean) => void;
  setDock: Dispatch<SetStateAction<AssistantDock | undefined>>;
  setProposalsCount: (count: number | undefined) => void;
  setCloseTo: (address: string | undefined) => void;
}

/** What screens tell the shell: stable for the shell's life, so effects keyed on it run once. */
function useActions({ setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo }: Setters): FrameActions {
  return useMemo<FrameActions>(
    () => ({
      mount: (slot) => {
        setCounts((now) => ({ ...now, [slot]: now[slot] + 1 }));
        return () => {
          setCounts((now) => ({ ...now, [slot]: now[slot] - 1 }));
        };
      },
      show: (next) => {
        setTab(next);
        setHidden(false);
      },
      dock: (next) => {
        setDock(next);
        return () => {
          setDock((now) => (now === next ? undefined : now));
        };
      },
      countProposals: (count) => {
        setProposalsCount(count);
        return () => {
          setProposalsCount(undefined);
        };
      },
      closeTo: setCloseTo,
    }),
    [setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo],
  );
}

export function useFrame(): Frame {
  const [inspector, setInspector] = useState<HTMLElement | null>(null);
  const [assistant, setAssistant] = useState<HTMLElement | null>(null);
  const [proposals, setProposals] = useState<HTMLElement | null>(null);
  const [proposalsCount, setProposalsCount] = useState<number | undefined>(undefined);
  const [counts, setCounts] = useState<Counts>({ inspector: 0, assistant: 0, proposals: 0 });
  const [tab, setTab] = useState<InspectorTab>("inspector");
  const [dock, setDock] = useState<AssistantDock | undefined>(undefined);
  const [hidden, setHidden] = useState(false);
  const [snap, setSnap] = useState<SheetSnap>("half");
  const [closeTo, setCloseTo] = useState<string | undefined>(undefined);
  const actions = useActions({ setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo });
  // A pane with nothing in it cannot be the tab in view.
  const order: InspectorTab[] = [tab, "inspector", "assistant", "proposals"];
  const shown: InspectorTab = order.find((each) => counts[each] > 0) ?? "inspector";
  // An offered assistant keeps the column, and so its tabs, on screen: they are its only way in.
  const present = counts.inspector > 0 || counts.assistant > 0 || counts.proposals > 0 || dock !== undefined;
  const toggleFolded = useCallback(() => {
    setHidden((now) => !now);
  }, []);
  useFrameKeys({ tab: shown, dock, closeTo }, toggleFolded);
  const openAssistant = useOpenAssistant(dock, actions);
  const state = useMemo<FrameState>(() => ({ inspector, assistant, proposals, proposalsCount, tab: shown, openAssistant }), [inspector, assistant, proposals, proposalsCount, shown, openAssistant]);
  // The inspector's tab is there where it has something, or the assistant's does (its tabs are its only way in), or nothing else is.
  const tabs = useMemo<InspectorTab[]>(
    () => [...(counts.inspector > 0 || dock !== undefined || counts.proposals === 0 ? (["inspector"] as const) : []), ...(dock === undefined ? [] : (["assistant"] as const)), ...(counts.proposals > 0 ? (["proposals"] as const) : [])],
    [dock, counts.inspector, counts.proposals],
  );
  return {
    state,
    actions,
    open: present && !hidden,
    present,
    folded: present && hidden,
    toggleFolded,
    snap,
    setSnap,
    dock,
    choose: (next) => {
      if (next === "assistant" && counts.assistant === 0) {
        dock?.open();
      }
      setTab(next);
    },
    setInspector,
    setAssistant,
    setProposals,
    tabs,
  };
}
