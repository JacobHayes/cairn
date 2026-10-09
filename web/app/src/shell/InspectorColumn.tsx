// The inspector column of the frame (design 3.1, 3.3): a graphite column on the right at
// 1100px and wider, a bottom sheet from 720 to 1099px (with a drag handle that snaps between
// two heights), and stacked under the workspace on a phone. Its head has two tabs, INSPECTOR
// and ASSISTANT, when the assistant is on offer, the sheet's grip in the middle, and the collapse
// (the column only) and close controls at the right. The panes are empty slots: the screens'
// content arrives through portals (frame.tsx). The inspector pane is the column's one
// scroller; the assistant pane holds its own (a transcript above a pinned composer).
import { useRef, type PointerEvent as ReactPointerEvent, type Ref, type RefObject } from "react";

import type { InspectorTab } from "./frame.tsx";

/** The sheet's two heights, as `data-snap` on the column (app.css sets each). */
export type SheetSnap = "half" | "full";

/** The status strip's height, which the sheet sits above (app.css, `--strip-h`). */
const STRIP_PX = 28;
/** Past this share of the window, a released drag snaps to the full height. */
const FULL_FROM = 0.62;

/** A drag on the sheet's handle: the height follows the pointer, and letting go snaps. */
function useSheetDrag(column: RefObject<HTMLElement | null>, snap: SheetSnap, onSnap: (snap: SheetSnap) => void) {
  // On the frame, not the column, so the workspace's inset above the sheet follows the drag (app.css).
  const frame = () => column.current?.closest<HTMLElement>(".frame");
  const start = useRef<{ moved: boolean } | undefined>(undefined);
  const heightAt = (clientY: number) => globalThis.innerHeight - clientY - STRIP_PX;
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

export interface InspectorColumnProps {
  open: boolean;
  tab: InspectorTab;
  snap: SheetSnap;
  /** The assistant can be opened: show the tab bar. */
  assistantOffered: boolean;
  onTab: (tab: InspectorTab) => void;
  onSnap: (snap: SheetSnap) => void;
  onCollapse: () => void;
  onClose: () => void;
  inspectorRef: Ref<HTMLDivElement>;
  assistantRef: Ref<HTMLDivElement>;
}

export function InspectorColumn({ open, tab, snap, assistantOffered, onTab, onSnap, onCollapse, onClose, inspectorRef, assistantRef }: InspectorColumnProps) {
  const column = useRef<HTMLElement>(null);
  const drag = useSheetDrag(column, snap, onSnap);
  return (
    <aside ref={column} className="inspector" aria-label="Inspector column" data-snap={snap} data-tab={tab} hidden={!open}>
      <div className="inspector-head">
        {assistantOffered ? (
          <div className="segmented on-graphite" role="tablist" aria-label="Inspector">
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
        <button type="button" className="ghost collapse" aria-label="Collapse the inspector" onClick={onCollapse}>
          ⤢
        </button>
        <button type="button" className="ghost" aria-label="Close" onClick={onClose}>
          ×
        </button>
      </div>
      <div ref={inspectorRef} className="inspector-body" data-pane="inspector" hidden={tab !== "inspector"} />
      <div ref={assistantRef} className="assistant-pane" data-pane="assistant" hidden={tab !== "assistant"} />
    </aside>
  );
}
