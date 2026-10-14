// The inspector column of the frame (design 3.1, 3.3): a chrome column on the right at
// 1100px and wider, a bottom sheet from 720 to 1099px (with a drag handle that snaps between
// two heights), and stacked under the workspace on a phone. Its head has two tabs, INSPECTOR
// and ASSISTANT, when the assistant is on offer, the sheet's grip in the middle, and one control
// at the right that hides it. Hidden, the column folds to a rail on the right edge (a slim bar
// at the bottom for the sheet) whose one button shows it again. The panes are empty slots: the
// screens' content arrives through portals (frame.tsx). The inspector pane is the column's one
// scroller; the assistant pane holds its own (a transcript above a pinned composer).
import { useRef, type PointerEvent as ReactPointerEvent, type Ref, type RefObject } from "react";

import type { InspectorTab } from "./frame.tsx";

/** The sheet's two heights, as `data-snap` on the column (app.css sets each). */
export type SheetSnap = "half" | "full";

/** Past this share of the window, a released drag snaps to the full height. */
const FULL_FROM = 0.62;

/** A drag on the sheet's handle: the height follows the pointer, and letting go snaps. */
function useSheetDrag(column: RefObject<HTMLElement | null>, snap: SheetSnap, onSnap: (snap: SheetSnap) => void) {
  // On the frame, not the column, so the workspace's inset above the sheet follows the drag (app.css).
  const frame = () => column.current?.closest<HTMLElement>(".frame");
  const start = useRef<{ moved: boolean } | undefined>(undefined);
  const heightAt = (clientY: number) => globalThis.innerHeight - clientY;
  return {
    onPointerDown: (event: ReactPointerEvent<HTMLButtonElement>) => {
      event.currentTarget.setPointerCapture(event.pointerId);
      start.current = { moved: false };
    },
    onPointerMove: (event: ReactPointerEvent<HTMLButtonElement>) => {
      if (start.current === undefined) {
        return;
      }
      start.current.moved = true;
      const height = Math.min(Math.max(heightAt(event.clientY), 96), globalThis.innerHeight * 0.9);
      frame()?.style.setProperty("--sheet-h", `${String(height)}px`);
    },
    onPointerUp: (event: ReactPointerEvent<HTMLButtonElement>) => {
      const dragged = start.current?.moved === true;
      start.current = undefined;
      frame()?.style.removeProperty("--sheet-h");
      if (dragged) {
        onSnap(heightAt(event.clientY) > globalThis.innerHeight * FULL_FROM ? "full" : "half");
      } else {
        onSnap(snap === "half" ? "full" : "half");
      }
    },
  };
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
  /** The assistant can be opened: show the tab bar. */
  assistantOffered: boolean;
  onTab: (tab: InspectorTab) => void;
  onSnap: (snap: SheetSnap) => void;
  onToggle: () => void;
  inspectorRef: Ref<HTMLDivElement>;
  assistantRef: Ref<HTMLDivElement>;
}

export function InspectorColumn({ present, folded, tab, snap, assistantOffered, onTab, onSnap, onToggle, inspectorRef, assistantRef }: InspectorColumnProps) {
  const column = useRef<HTMLElement>(null);
  const drag = useSheetDrag(column, snap, onSnap);
  return (
    <aside ref={column} className="inspector" aria-label="Inspector column" data-snap={snap} data-tab={tab} data-folded={folded} hidden={!present}>
      <button type="button" className="inspector-rail" aria-label="Show inspector" title="Show the inspector  ]" hidden={!folded} onClick={onToggle}>
        <FoldIcon toward="left" />
        <span className="rail-label">Inspector</span>
      </button>
      <div className="inspector-head" hidden={folded}>
        {assistantOffered ? (
          <div className="segmented on-chrome" role="tablist" aria-label="Inspector">
            {(["inspector", "assistant"] as const).map((each) => (
              <button
                key={each}
                type="button"
                role="tab"
                aria-selected={tab === each}
                data-testid={`tab-${each}`}
                onClick={() => {
                  onTab(each);
                }}
              >
                {each}
              </button>
            ))}
          </div>
        ) : (
          <span className="label">Inspector</span>
        )}
        <button type="button" className="sheet-handle" aria-label="Resize the inspector" {...drag} />
        <span className="spacer" />
        <button type="button" className="ghost hide" aria-label="Hide inspector" title="Hide the inspector  ]" onClick={onToggle}>
          <FoldIcon toward="right" />
        </button>
      </div>
      <div ref={inspectorRef} className="inspector-body" data-pane="inspector" hidden={folded || tab !== "inspector"} />
      <div ref={assistantRef} className="assistant-pane" data-pane="assistant" hidden={folded || tab !== "assistant"} />
    </aside>
  );
}
