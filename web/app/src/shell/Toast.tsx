// The Graticule toast, kept only for confirmations that are not about syncing ("Route file
// exported", design 9.7): it takes itself down after a few seconds, waits while the pointer is
// on it, has no close button, and the next one replaces it. It rises at the workspace's
// bottom right, above the strip, so it never covers the sync chip (app.css).
import { useEffect, useState } from "react";

import { useSession, useToast } from "../data/react.ts";

/** How long a toast stays; a problem stays longer, since it says what to do next. */
export const TOAST_MS = { confirm: 2500, problem: 6000 };

export function Toast() {
  const toast = useToast();
  const { activity } = useSession();
  const [hovered, setHovered] = useState(false);
  const id = toast?.id;
  const ms = toast === undefined ? 0 : TOAST_MS[toast.tone];
  useEffect(() => {
    if (id === undefined || hovered) {
      return undefined;
    }
    const timer = setTimeout(() => {
      activity.clearToast(id);
    }, ms);
    return () => {
      clearTimeout(timer);
    };
  }, [activity, id, ms, hovered]);
  return (
    <div className="toasts" aria-live="polite">
      {toast === undefined ? null : (
        <div
          key={toast.id}
          className="toast show"
          data-testid="toast"
          data-tone={toast.tone}
          onPointerEnter={() => {
            setHovered(true);
          }}
          onPointerLeave={() => {
            setHovered(false);
          }}
        >
          {toast.text}
        </div>
      )}
    </div>
  );
}
