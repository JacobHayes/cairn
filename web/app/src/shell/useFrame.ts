// The frame's state: which of the inspector column's panes have something in them, which tab
// is in view, whether the viewer hid the column (`]`), and where Esc takes the sheet.
// The screens report through `FrameActions` (frame.tsx); the shell reads the result.
import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { useNavigate } from "react-router";

import type { SheetSnap } from "./InspectorColumn.tsx";
import { overlayOpen, typing } from "../ui/typing.ts";
import { COLUMN, PEEK_FIRST, SHEET, useMedia, type AssistantDock, type FrameActions, type FrameState, type InspectorTab } from "./frame.tsx";

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
 * `]` hides and shows the inspector; Esc steps back out of the sheet (below 1100px), unless a field
 * has focus: the assistant when its tab is in view, else the open node by going back to the screen
 * it is open on (`closeTo`), else the sheet itself closes.
 */
function useFrameKeys({ tab, dock, closeTo, open }: { tab: InspectorTab; dock: AssistantDock | undefined; closeTo: string | undefined; open: boolean }, fold: () => void): void {
  const navigate = useNavigate();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target) || overlayOpen()) {
        return;
      }
      if (event.key === "]") {
        fold();
      } else if (event.key === "Escape" && globalThis.matchMedia(SHEET).matches) {
        if (tab === "assistant") {
          dock?.close();
        } else if (closeTo !== undefined) {
          void navigate(closeTo);
        } else if (open) {
          fold();
        }
      }
    };
    // In the capture phase, so a menu or dialog that closes on this Esc is still there to be seen, and the sheet stays.
    globalThis.addEventListener("keydown", onKey, true);
    return () => {
      globalThis.removeEventListener("keydown", onKey, true);
    };
  }, [tab, dock, closeTo, open, navigate, fold]);
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
  /** Shows the column (a phone's sheet opens at its peek) or hides it. */
  setHidden: (hidden: boolean) => void;
  setDock: Dispatch<SetStateAction<AssistantDock | undefined>>;
  setProposalsCount: (count: number | undefined) => void;
  setCloseTo: (address: string | undefined) => void;
  setSnap: (snap: SheetSnap) => void;
}

/** What screens tell the shell: stable for the shell's life, so effects keyed on it run once. */
function useActions({ setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo, setSnap }: Setters): FrameActions {
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
      snap: setSnap,
    }),
    [setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo, setSnap],
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
  const [sheet, setSheet] = useState<{ hidden: boolean; snap: SheetSnap }>(() => ({ hidden: false, snap: globalThis.matchMedia(PEEK_FIRST).matches ? "peek" : "half" }));
  const { hidden, snap } = sheet;
  const setSnap = useCallback((next: SheetSnap) => {
    setSheet((now) => ({ ...now, snap: next }));
  }, []);
  // A phone's (or a short window's) sheet opens at its peek, whatever height it was closed at: picking a node shows its tabs and leaves the graph usable.
  const setHidden = useCallback((next: boolean) => {
    setSheet((now) => (now.hidden === next ? now : { hidden: next, snap: !next && globalThis.matchMedia(PEEK_FIRST).matches ? "peek" : now.snap }));
  }, []);
  // Widening to a column and narrowing back to a sheet (a resize, a rotation) starts the sheet over at its peek; its heights are CSS's, so they follow the window by themselves.
  const wide = useMedia(COLUMN);
  const wasWide = useRef(wide);
  useEffect(() => {
    if (wasWide.current && !wide) {
      setSnap("peek");
    }
    wasWide.current = wide;
  }, [wide, setSnap]);
  const [closeTo, setCloseTo] = useState<string | undefined>(undefined);
  const actions = useActions({ setCounts, setTab, setHidden, setDock, setProposalsCount, setCloseTo, setSnap });
  // A pane with nothing in it cannot be the tab in view.
  const order: InspectorTab[] = [tab, "inspector", "assistant", "proposals"];
  const shown: InspectorTab = order.find((each) => counts[each] > 0) ?? "inspector";
  // An offered assistant keeps the column, and so its tabs, on screen: they are its only way in.
  const present = counts.inspector > 0 || counts.assistant > 0 || counts.proposals > 0 || dock !== undefined;
  const toggleFolded = useCallback(() => {
    setSheet((now) => ({ hidden: !now.hidden, snap: now.hidden && globalThis.matchMedia(PEEK_FIRST).matches ? "peek" : now.snap }));
  }, []);
  useFrameKeys({ tab: shown, dock, closeTo, open: present && !hidden }, toggleFolded);
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
      // A tab picked at the peek shows nothing of its pane: the sheet opens to half.
      setSheet((now) => (now.snap === "peek" ? { ...now, snap: "half" } : now));
    },
    setInspector,
    setAssistant,
    setProposals,
    tabs,
  };
}
