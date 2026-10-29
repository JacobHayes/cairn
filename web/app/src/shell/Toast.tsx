// The Graticule toast, kept only for confirmations that are not about syncing ("Route file
// exported"): it takes itself down after a few seconds, waits while the pointer is
// on it, has no close button, and the next one replaces it. It rises at the workspace's
// bottom right (app.css). The timing is the toast machine's (data/toasts.ts).
import { useSelector } from "@xstate/react";

import { useSession } from "../data/react.ts";
import { toastOf } from "../data/toasts.ts";

export function Toast() {
  const { toasts } = useSession();
  const toast = useSelector(toasts, toastOf);
  return (
    <div className="toasts" aria-live="polite">
      {toast === undefined ? null : (
        <div
          key={toast.id}
          className="toast show"
          data-testid="toast"
          data-tone={toast.tone}
          onPointerEnter={() => {
            toasts.send({ type: "hover" });
          }}
          onPointerLeave={() => {
            toasts.send({ type: "leave" });
          }}
        >
          {toast.text}
        </div>
      )}
    </div>
  );
}
