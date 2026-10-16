// The frame's state: which of the inspector column's panes have something in them, which tab
// is in view, whether the viewer hid the column (`]`), and where Esc takes the sheet.
// The screens report through `FrameActions` (frame.tsx); the shell reads the result.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import type { SheetSnap } from "./InspectorColumn.tsx";
import { typing } from "../ui/typing.ts";
import { PHONE_WIDTH, SHEET, useMedia, type AssistantDock, type FrameActions, type FrameState, type InspectorTab } from "./frame.tsx";

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

export function useFrame(): Frame {
  const [inspector, setInspector] = useState<HTMLElement | null>(null);
  const [assistant, setAssistant] = useState<HTMLElement | null>(null);
  const [counts, setCounts] = useState({ inspector: 0, assistant: 0 });
  const [tab, setTab] = useState<InspectorTab>("inspector");
  const [dock, setDock] = useState<AssistantDock | undefined>(undefined);
  const [hidden, setHidden] = useState(false);
  const [snap, setSnap] = useState<SheetSnap>("half");
  const [closeTo, setCloseTo] = useState<string | undefined>(undefined);
  const actions = useMemo<FrameActions>(
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
      closeTo: setCloseTo,
    }),
    [],
  );
  // A pane with nothing in it cannot be the tab in view.
  const shown: InspectorTab = counts.assistant > 0 && (counts.inspector === 0 || tab === "assistant") ? "assistant" : "inspector";
  // An offered assistant keeps the column, and so its tabs, on screen: they are its only way in.
  const present = counts.inspector > 0 || counts.assistant > 0 || dock !== undefined;
  // A phone stacks the inspector under the page, except over the full-screen map, where it is a sheet that can be hidden.
  const stacked = useMedia(PHONE_WIDTH) && new URLSearchParams(useLocation().search).get("map") !== "1";
  const toggleFolded = useCallback(() => {
    setHidden((now) => !now);
  }, []);
  useFrameKeys({ tab: shown, dock, closeTo }, toggleFolded);
  const state = useMemo<FrameState>(() => ({ inspector, assistant, tab: shown }), [inspector, assistant, shown]);
  return {
    state,
    actions,
    // The stacked inspector has no way to hide it.
    open: present && (!hidden || stacked),
    present,
    folded: present && hidden && !stacked,
    toggleFolded,
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
