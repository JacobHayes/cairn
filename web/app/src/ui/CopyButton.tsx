// Copy beside a value: writes `text` to the clipboard and says so on the button for two
// seconds, or says it could not (a page without clipboard access, a refused permission), so the
// person selects the text instead. Its accessible name says what it copies.
import { useEffect, useState } from "react";

import { Button } from "./kit.tsx";

/** How long the button keeps saying what happened. */
export const COPY_FEEDBACK_MS = 2000;

/** What the last copy did: a new object for each click, so a repeat restarts the time. */
type Outcome = { what: "copied" | "failed" } | undefined;

export function CopyButton({ what, text }: { what: string; text: string }) {
  const [outcome, setOutcome] = useState<Outcome>(undefined);
  useEffect(() => {
    if (outcome === undefined) {
      return undefined;
    }
    const timer = setTimeout(() => {
      setOutcome(undefined);
    }, COPY_FEEDBACK_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [outcome]);
  const copy = () => {
    // A page without clipboard access has no `navigator.clipboard` at all.
    const written = typeof navigator.clipboard === "undefined" ? Promise.reject(new Error("no clipboard")) : navigator.clipboard.writeText(text);
    written.then(
      () => {
        setOutcome({ what: "copied" });
      },
      () => {
        setOutcome({ what: "failed" });
      },
    );
  };
  const said = outcome?.what === "copied" ? "Copied" : outcome?.what === "failed" ? "Copy failed" : undefined;
  return (
    <>
      <Button onClick={copy} aria-label={`Copy ${what}`} title={`Copy ${what} to the clipboard`} data-outcome={outcome?.what}>
        {said ?? "Copy"}
      </Button>
      {/* Heard, not only seen: the button's name stays the thing it copies. */}
      <span className="visually-hidden" role="status">
        {said === undefined ? "" : `${said}: ${what}`}
      </span>
    </>
  );
}
