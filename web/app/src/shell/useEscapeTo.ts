// Esc closes what the inspector holds, on the widths where the shell does not (it closes the
// tablet's sheet itself, frame.tsx): the open node or item goes back to the screen it was opened
// from, unless a field has focus or a popover or dialog is open (it closes first).
import { useEffect } from "react";
import { useNavigate } from "react-router";

import { typing } from "../ui/typing.ts";
import { SHEET } from "./frame.tsx";

/** Esc goes to `address`, while there is one (undefined: nothing is open). */
export function useEscapeTo(address: string | undefined): void {
  const navigate = useNavigate();
  useEffect(() => {
    if (address === undefined) {
      return undefined;
    }
    const heard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !typing(event.target) && !globalThis.matchMedia(SHEET).matches && document.querySelector("[data-popover], dialog[open]") === null) {
        void navigate(address);
      }
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [navigate, address]);
}
