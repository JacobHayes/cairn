// `?`: the key sheet, a modal with a table of the bindings (3.4). Escape or Close shuts it.
// It takes the focus while it is open (Tab stays on its Close button, which is all it holds),
// so Enter or Space cannot reach the control that had the focus behind it, and gives the
// focus back on close.
import { useEffect, useRef } from "react";

import { Button } from "../ui/kit.tsx";
import { KEY_BINDINGS } from "./keys.ts";

export function KeySheet({ onClose }: { onClose: () => void }) {
  const sheet = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const before = document.activeElement;
    sheet.current?.querySelector("button")?.focus();
    return () => {
      if (before instanceof HTMLElement) {
        before.focus();
      }
    };
  }, []);
  return (
    <div
      ref={sheet}
      className="key-sheet panel stack"
      role="dialog"
      aria-modal="true"
      aria-label="Keys"
      data-testid="key-sheet"
      onKeyDown={(event) => {
        if (event.key === "Tab") {
          event.preventDefault();
        }
      }}
    >
      <span className="row">
        <strong>Keys</strong>
        <span className="spacer" />
        <Button onClick={onClose}>Close</Button>
      </span>
      <table className="data">
        <tbody>
          {KEY_BINDINGS.map((binding) => (
            <tr key={binding.keys}>
              <td>
                <kbd>{binding.keys}</kbd>
              </td>
              <td>{binding.words}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <span className="muted small">Keys never fire while a field has focus.</span>
    </div>
  );
}
