// The frame's state: which of the inspector column's panes have something in them, which tab
// is in view, whether the viewer collapsed the column (`]`), and where Esc takes the sheet.
// The screens report through `FrameActions` (frame.tsx); the shell reads the result.
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";

import type { SheetSnap } from "./InspectorColumn.tsx";
import { useNarrow } from "../canvas/MapFrame.tsx";
import { SHEET, type AssistantDock, type FrameActions, type FrameState, type InspectorTab } from "./frame.tsx";

/** Whether the keystroke is for a field, which keys never fire in (design 3.4). */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

export interface Frame {
  state: FrameState;
  actions: FrameActions;
  /** The column is on screen: something is in a pane and the viewer has not collapsed it. */
  open: boolean;
  /** Something is in a pane, collapsed or not. */
  present: boolean;
  collapsed: boolean;
  toggleCollapsed: () => void;
  /** Closes what the column shows: the open node (back to the screen it is on) or the assistant. */
  close: () => void;
  snap: SheetSnap;
  setSnap: (snap: SheetSnap) => void;
  dock: AssistantDock | undefined;
  /** The tab bar's choice: the assistant's tab opens the assistant when it is closed. */
  choose: (tab: InspectorTab) => void;
  setInspector: (element: HTMLElement | null) => void;
  setAssistant: (element: HTMLElement | null) => void;
}

/** What Esc closes: the sheet in view, which is the node's (back to the screen it is open on, `address`) or the assistant's. */
interface Closable {
  tab: InspectorTab;
  dock: AssistantDock | undefined;
}

/** `]` collapses the inspector; Esc closes the tablet's sheet, unless a field has focus. */
function useFrameKeys(onClose: { current: () => void }, collapse: () => void): void {
  const toggle = useRef(collapse);
  toggle.current = collapse;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || typing(event.target)) {
        return;
      }
      if (event.key === "]") {
        toggle.current();
      } else if (event.key === "Escape" && globalThis.matchMedia(SHEET).matches) {
        onClose.current();
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  }, [onClose]);
}

/** Closes what the column shows: the assistant, or the open node by going back to the screen it is open on. */
function useCloseShown(address: { current: string | undefined }, closable: { current: Closable }): { current: () => void } {
  const navigate = useNavigate();
  const closeShown = useRef(() => {});
  closeShown.current = () => {
    const { tab, dock } = closable.current;
    if (tab === "assistant") {
      dock?.close();
    } else if (address.current !== undefined) {
      void navigate(address.current);
    }
  };
  return closeShown;
}

export function useFrame(): Frame {
  const [inspector, setInspector] = useState<HTMLElement | null>(null);
  const [assistant, setAssistant] = useState<HTMLElement | null>(null);
  const [counts, setCounts] = useState({ inspector: 0, assistant: 0 });
  const [tab, setTab] = useState<InspectorTab>("inspector");
  const [dock, setDock] = useState<AssistantDock | undefined>(undefined);
  const [collapsed, setCollapsed] = useState(false);
  const [snap, setSnap] = useState<SheetSnap>("half");
  const close = useRef<string | undefined>(undefined);
  const actions = useMemo<FrameActions>(
    () => ({
      mount: (slot) => {
        setCounts((now) => ({ ...now, [slot]: now[slot] + 1 }));
        return () => {
          setCounts((now) => ({ ...now, [slot]: now[slot] - 1 }));
        };
      },
      show: setTab,
      dock: (next) => {
        setDock(next);
        return () => {
          setDock((now) => (now === next ? undefined : now));
        };
      },
      closeTo: (address) => {
        close.current = address;
      },
    }),
    [],
  );
  // A pane with nothing in it cannot be the tab in view.
  const shown: InspectorTab = counts.assistant > 0 && (counts.inspector === 0 || tab === "assistant") ? "assistant" : "inspector";
  const present = counts.inspector > 0 || counts.assistant > 0;
  const narrow = useNarrow();
  const closable = useRef<Closable>({ tab: "inspector", dock: undefined });
  closable.current = { tab: shown, dock };
  const closeShown = useCloseShown(close, closable);
  useFrameKeys(closeShown, () => {
    setCollapsed((now) => !now);
  });
  const state = useMemo<FrameState>(() => ({ inspector, assistant, tab: shown }), [inspector, assistant, shown]);
  return {
    state,
    actions,
    // A phone stacks the inspector under the page and has no way to expand a collapsed one.
    open: present && (!collapsed || narrow),
    present,
    collapsed,
    toggleCollapsed: () => {
      setCollapsed((now) => !now);
    },
    close: () => {
      closeShown.current();
    },
    snap,
    setSnap,
    dock,
    choose: (next) => {
      if (next === "assistant" && counts.assistant === 0) {
        dock?.open();
      } else {
        setTab(next);
      }
    },
    setInspector,
    setAssistant,
  };
}
