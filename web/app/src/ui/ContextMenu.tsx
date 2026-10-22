// A menu of actions opened at a point (a right-click or the keyboard's menu key on a thing):
// groups of items with hairline dividers between them and an optional mono label over a group.
// Arrow keys move, Home and End jump, Enter or Space chooses, Escape or a press outside closes
// and hands focus back to where it was. It is marked `data-popover`, so the keyboard map's
// Escape leaves the selection alone while it is open, and it is drawn on the page, not inside
// whatever transformed surface its opener sits in.
import { useEffect, useLayoutEffect, useRef, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { createPortal } from "react-dom";

import { menuFocusTarget } from "./menuKeys.ts";
import { place, windowBounds } from "./place.ts";

/** The space a menu keeps from the window's edge (px). */
const EDGE_PX = 8;

export interface ContextItem {
  id: string;
  label: string;
}

export interface ContextGroup {
  /** A mono label over the group. */
  label?: string;
  items: ContextItem[];
}

function Group({ group, first, onChoose }: { group: ContextGroup; first: boolean; onChoose: (id: string) => void }) {
  return (
    <div role="group" aria-label={group.label}>
      {first ? null : <hr />}
      {group.label === undefined ? null : <div className="mlabel">{group.label}</div>}
      {group.items.map((item) => (
        <button
          key={item.id}
          type="button"
          role="menuitem"
          className="menu-item"
          data-testid={`context-${item.id}`}
          onClick={() => {
            onChoose(item.id);
          }}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

export function ContextMenu({ at, groups, label, onChoose, onClose }: { at: { x: number; y: number }; groups: ContextGroup[]; label: string; onChoose: (id: string) => void; onClose: () => void }) {
  const root = useRef<HTMLDivElement>(null);
  // Where focus was when it opened, so closing hands it back.
  const opener = useRef(document.activeElement);
  const items = () => [...(root.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
  useLayoutEffect(() => {
    const box = root.current?.getBoundingClientRect();
    if (root.current === null || box === undefined) {
      return;
    }
    // At the pointer, slid back inside the window when it is near the right or lower edge.
    const placed = place({ left: at.x, top: at.y, width: 0, height: 0 }, box, windowBounds(), { side: "bottom", align: "start", gap: 0, margin: EDGE_PX });
    root.current.style.setProperty("left", `${String(placed.left)}px`);
    root.current.style.setProperty("top", `${String(placed.top)}px`);
    items()[0]?.focus();
  }, [at]);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && root.current?.contains(event.target) !== true) {
        onClose();
      }
    };
    document.addEventListener("pointerdown", outside);
    const back = opener.current;
    return () => {
      document.removeEventListener("pointerdown", outside);
      if (back instanceof HTMLElement && document.activeElement === document.body) {
        back.focus();
      }
    };
  }, [onClose]);
  const onKeyDown = (event: ReactKeyboardEvent) => {
    const all = items();
    const to = menuFocusTarget(event.key, all.indexOf(document.activeElement as HTMLElement), all.length);
    if (to !== undefined) {
      event.preventDefault();
      all[to]?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "Tab") {
      onClose();
    }
  };
  return createPortal(
    <div ref={root} className="popover context-menu" role="menu" aria-label={label} data-popover="" data-overhang="" data-testid="context-menu" onKeyDown={onKeyDown} onContextMenu={(event) => { event.preventDefault(); }}>
      <div className="popover-body">
        {groups.map((group, index) => (
          <Group key={index} group={group} first={index === 0} onChoose={(id) => { onClose(); onChoose(id); }} />
        ))}
      </div>
    </div>,
    document.body,
  );
}
