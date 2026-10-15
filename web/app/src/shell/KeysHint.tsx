// A quiet "?" button on the rail's "You" line, present on every screen. Hovering or focusing it
// lists the keys in a popover; `?` (on a journey) still opens the full sheet. The popover is
// fixed, placed from the hint's box, so the rail's scroller does not clip it; it abuts the
// hint (under it when the rail is the phone's top bar) so the pointer can cross onto it.
import { useEffect, useState, type CSSProperties } from "react";

import { KeyTable } from "../screens/KeySheet.tsx";

function placeBeside(hint: HTMLElement): CSSProperties {
  const box = hint.getBoundingClientRect();
  // Beside the hint, slid back inside a narrow window; under it when it sits in the top half.
  const left = Math.max(12, Math.min(box.right, globalThis.innerWidth - 360 - 12));
  return box.top < globalThis.innerHeight / 2 ? { top: box.bottom, left } : { bottom: globalThis.innerHeight - box.bottom, left };
}

export function KeysHint() {
  const [place, setPlace] = useState<CSSProperties | undefined>(undefined);
  const show = (hint: HTMLElement) => {
    setPlace(placeBeside(hint));
  };
  const hide = () => {
    setPlace(undefined);
  };
  // Esc closes the hint first, however it was opened, and goes no further (the selection stays).
  const open = place !== undefined;
  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setPlace(undefined);
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open]);
  return (
    <span className="keys-hint" onMouseLeave={hide} onBlur={hide}>
      <button
        type="button"
        className="ghost keys-hint-button"
        aria-label="Keyboard shortcuts"
        title="Keyboard shortcuts (?)"
        aria-describedby={place === undefined ? undefined : "keys-hint-list"}
        onMouseEnter={(event) => { show(event.currentTarget); }}
        onFocus={(event) => { show(event.currentTarget); }}
      >
        ?
      </button>
      {place === undefined ? null : (
        <div id="keys-hint-list" role="tooltip" className="popover keys-popover" style={place} data-testid="keys-hint">
          <div className="popover-head">Keys</div>
          <div className="popover-body">
            <KeyTable />
          </div>
        </div>
      )}
    </span>
  );
}
