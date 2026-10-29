// The inspector column of the frame: a chrome column on the right at
// 1100px and wider, and below that, tablet and phone alike, a bottom sheet over the workspace. The
// sheet has a handle that drags (touch or pointer), taps and answers the arrow keys between three
// heights, peek (the head and the top of the pane), half and full; letting go below the peek
// closes it. Its head has a tab for each pane on offer, Inspector, Assistant (where the assistant
// is offered) and Proposals (a journey's or route's open proposals), in sentence case as DESIGN's
// tabs are, and one control at the right that hides it. Hidden, the column folds to a rail on the
// right edge (a labelled pill at the bottom for the sheet, and nothing else: no bar, no blank
// strip) that shows it again, with a vertical tab for each pane that opens it on that tab, in the
// rail's uppercase mono (app.css). The panes are empty slots: the screens' content arrives through
// portals (frame.tsx). The inspector pane is the column's one scroller; the assistant pane holds
// its own (a transcript above a pinned composer).
import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type Ref, type RefObject } from "react";

import { COLUMN, useMedia, type InspectorTab } from "./frame.tsx";
import { Tip } from "../ui/Tip.tsx";

/** The sheet's heights, as `data-sheet` on the frame (app.css sets each), smallest first. */
export const SNAPS = ["peek", "half", "full"] as const;
export type SheetSnap = (typeof SNAPS)[number];

/** A pointer that moves less than this (px) between down and up is a tap, not a drag. */
const TAP_SLOP = 6;
/** The shortest the sheet is dragged to (px), and how far under the peek a release closes it. */
const DRAG_FLOOR = 40;
const CLOSE_BELOW = 0.7;

/** The next snap up or down from `snap`, or `snap` at the end. */
function stepped(snap: SheetSnap, by: 1 | -1): SheetSnap {
  return SNAPS[Math.min(SNAPS.length - 1, Math.max(0, SNAPS.indexOf(snap) + by))] ?? snap;
}

/** The height of the sheet at each snap, smallest first: CSS's, read by trying each in turn before the next frame, so there is one source for them. */
function measureSnaps(frame: HTMLElement | undefined, column: HTMLElement): number[] {
  const was = frame?.dataset["sheet"];
  const heights = SNAPS.map((each) => {
    if (frame !== undefined) {
      frame.dataset["sheet"] = each;
    }
    return column.offsetHeight;
  });
  if (frame !== undefined && was !== undefined) {
    frame.dataset["sheet"] = was;
  }
  return heights;
}

/** A drag in progress: where it began, the sheet's height then, each snap's height, and whether the pointer has moved past a tap. */
interface Drag {
  startY: number;
  startHeight: number;
  heights: number[];
  moved: boolean;
}

/** The sheet's height for a pointer at `clientY`: it follows the pointer, between the floor and the tallest snap. */
function draggedHeight(drag: Drag, clientY: number): number {
  return Math.min(Math.max(drag.startHeight + drag.startY - clientY, DRAG_FLOOR), drag.heights[drag.heights.length - 1] ?? drag.startHeight);
}

/** Where a released drag settles: the snap nearest its height, or none (closed) from under the peek. */
function settledSnap(drag: Drag, height: number): SheetSnap | undefined {
  if (height < (drag.heights[0] ?? 0) * CLOSE_BELOW) {
    return undefined;
  }
  const nearest = drag.heights.reduce((best, each, at) => (Math.abs(each - height) < Math.abs((drag.heights[best] ?? 0) - height) ? at : best), 0);
  return SNAPS[nearest];
}

/**
 * The sheet's handle: a drag moves the sheet's height with the pointer and letting go snaps to the
 * nearest height (or closes it, from under the peek), a tap or Enter toggles the peek and the full
 * height, and the arrow keys step.
 */
function useSheetHandle(column: RefObject<HTMLElement | null>, snap: SheetSnap, onSnap: (snap: SheetSnap) => void, onClose: () => void) {
  // On the frame, not the column, so the workspace's inset above the sheet follows the drag (app.css).
  const frame = () => column.current?.closest<HTMLElement>(".frame") ?? undefined;
  const drag = useRef<Drag | undefined>(undefined);
  const swallowClick = useRef(false);
  const release = () => {
    frame()?.style.removeProperty("--sheet-h");
    drag.current = undefined;
  };
  return {
    onPointerDown: (event: ReactPointerEvent<HTMLButtonElement>) => {
      const element = column.current;
      if (element === null) {
        return;
      }
      event.currentTarget.setPointerCapture(event.pointerId);
      swallowClick.current = false;
      drag.current = { startY: event.clientY, startHeight: element.offsetHeight, heights: measureSnaps(frame(), element), moved: false };
    },
    onPointerMove: (event: ReactPointerEvent<HTMLButtonElement>) => {
      const now = drag.current;
      if (now === undefined || (!now.moved && Math.abs(event.clientY - now.startY) < TAP_SLOP)) {
        return;
      }
      now.moved = true;
      frame()?.style.setProperty("--sheet-h", `${String(draggedHeight(now, event.clientY))}px`);
    },
    onPointerUp: (event: ReactPointerEvent<HTMLButtonElement>) => {
      const now = drag.current;
      release();
      if (now === undefined || !now.moved) {
        return;
      }
      swallowClick.current = true;
      const settled = settledSnap(now, draggedHeight(now, event.clientY));
      if (settled === undefined) {
        onClose();
      } else {
        onSnap(settled);
      }
    },
    onPointerCancel: release,
    onClick: () => {
      if (swallowClick.current) {
        swallowClick.current = false;
        return;
      }
      onSnap(snap === "full" ? "peek" : "full");
    },
    onKeyDown: (event: ReactKeyboardEvent<HTMLButtonElement>) => {
      if (event.key === "ArrowUp" || event.key === "ArrowDown") {
        event.preventDefault();
        onSnap(stepped(snap, event.key === "ArrowUp" ? 1 : -1));
      }
    },
  };
}

/** Hiding moves focus to the control that shows the column, and showing moves it back to the tab in view, so a keyboard is never left on something gone. */
function useFocusFollowsFold(column: RefObject<HTMLElement | null>, show: RefObject<HTMLElement | null>, folded: boolean): void {
  const wasFolded = useRef(folded);
  useEffect(() => {
    const element = column.current;
    if (element !== null && wasFolded.current !== folded && (document.activeElement === document.body || element.contains(document.activeElement))) {
      (folded ? show.current : element.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]'))?.focus();
    }
    wasFolded.current = folded;
  }, [column, show, folded]);
}

/** A chevron against a bar: `toward` is the way it points, the bar sits at the edge the column folds to. */
function FoldIcon({ toward }: { toward: "right" | "left" }) {
  const flip = toward === "left";
  return (
    <svg className="fold-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d={flip ? "M10 3.5 5.5 8l4.5 4.5" : "M6 3.5 10.5 8 6 12.5"} />
      <path d={flip ? "M2.5 3v10" : "M13.5 3v10"} />
    </svg>
  );
}

export interface InspectorColumnProps {
  /** Something is in a pane. */
  present: boolean;
  /** The viewer hid it: only the rail (or the sheet's bar) shows. */
  folded: boolean;
  tab: InspectorTab;
  snap: SheetSnap;
  /** The tabs on offer, in order; the bar shows when there is more than one. */
  tabs: readonly InspectorTab[];
  /** How many open proposals the Proposals tab holds, once read. */
  proposalsCount: number | undefined;
  onTab: (tab: InspectorTab) => void;
  onSnap: (snap: SheetSnap) => void;
  /** Hides or shows the column (the sheet's close, and the pill that opens it). */
  onToggle: () => void;
  inspectorRef: Ref<HTMLDivElement>;
  assistantRef: Ref<HTMLDivElement>;
  proposalsRef: Ref<HTMLDivElement>;
}

const TAB_WORDS: Record<InspectorTab, string> = { inspector: "Inspector", assistant: "Assistant", proposals: "Proposals" };

/** A tab's name, with the count of open proposals in mono beside Proposals when there are any. */
function TabName({ tab, count }: { tab: InspectorTab; count: number | undefined }) {
  return (
    <>
      {TAB_WORDS[tab]}
      {tab === "proposals" && count !== undefined && count > 0 ? <span className="tab-count" data-testid="proposals-count">{count}</span> : null}
    </>
  );
}

/** What shows of the column once hidden: the button that shows it and, on the wide rail, a vertical tab for each pane. */
function FoldedRail({ folded, tab, tabs, proposalsCount, show, onTab, onToggle }: Pick<InspectorColumnProps, "folded" | "tab" | "tabs" | "proposalsCount" | "onTab" | "onToggle"> & { show: Ref<HTMLButtonElement> }) {
  return (
    <div className="inspector-rail" hidden={!folded}>
      <Tip tip="Show the inspector" keys="]" kind="name">
        <button ref={show} type="button" className="rail-show" aria-label="Show inspector" onClick={onToggle}>
          <FoldIcon toward="left" />
          <span className="rail-show-label">Inspector</span>
        </button>
      </Tip>
      {tabs.map((each) => (
        <button
          key={each}
          type="button"
          className="rail-tab"
          data-testid={`rail-tab-${each}`}
          data-on={tab === each}
          onClick={() => {
            onTab(each);
            onToggle();
          }}
        >
          <TabName tab={each} count={proposalsCount} />
        </button>
      ))}
    </div>
  );
}

export function InspectorColumn({ present, folded, tab, snap, tabs, proposalsCount, onTab, onSnap, onToggle, inspectorRef, assistantRef, proposalsRef }: InspectorColumnProps) {
  const column = useRef<HTMLElement>(null);
  const handle = useSheetHandle(column, snap, onSnap, onToggle);
  const show = useRef<HTMLButtonElement>(null);
  // At the peek only the top of the pane shows: it is not read or reached there, and a tap on it opens the sheet to half.
  const wide = useMedia(COLUMN);
  const peeking = snap === "peek" && !wide;
  useFocusFollowsFold(column, show, folded);
  return (
    <aside ref={column} className="inspector" aria-label="Inspector column" data-tab={tab} data-folded={folded} hidden={!present} onClick={(event) => {
        if (peeking && event.target === event.currentTarget) {
          onSnap("half");
        }
      }}>
      <FoldedRail folded={folded} tab={tab} tabs={tabs} proposalsCount={proposalsCount} show={show} onTab={onTab} onToggle={onToggle} />
      <div className="inspector-head" hidden={folded}>
        <Tip tip="Drag to resize; Enter toggles; arrows step">
          <button type="button" className="sheet-handle" aria-label="Resize the inspector" {...handle} />
        </Tip>
        {tabs.length > 1 || tabs[0] === "proposals" ? (
          <div className="segmented on-chrome" role="tablist" aria-label="Inspector">
            {tabs.map((each) => (
              <button
                key={each}
                type="button"
                role="tab"
                aria-selected={tab === each}
                data-testid={`tab-${each}`}
                {...(each === "assistant" ? { "data-tour": "assistant" } : {})}
                onClick={() => {
                  onTab(each);
                }}
              >
                <TabName tab={each} count={proposalsCount} />
              </button>
            ))}
          </div>
        ) : (
          <span className="label">Inspector</span>
        )}
        <span className="spacer" />
        <Tip tip="Hide the inspector" keys="]" kind="name">
          <button type="button" className="ghost hide" aria-label="Hide inspector" onClick={onToggle}>
            <FoldIcon toward="right" />
          </button>
        </Tip>
      </div>
      <div ref={inspectorRef} className="inspector-body" data-pane="inspector" hidden={folded || tab !== "inspector"} inert={peeking} />
      <div ref={assistantRef} className="assistant-pane" data-pane="assistant" hidden={folded || tab !== "assistant"} inert={peeking} />
      <div ref={proposalsRef} className="inspector-body" data-pane="proposals" hidden={folded || tab !== "proposals"} inert={peeking} />
    </aside>
  );
}
