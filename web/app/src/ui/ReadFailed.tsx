// A read that failed, said to a person: one sentence for the kind of failure (never the server's
// own message, which may name internal ids), and the raw message behind a "Details" button for
// whoever needs to report it (e2e/internal-ids.ts fails a screen that shows an id).
import { useState } from "react";

import type { ReadError } from "../data/keyed-read.ts";
import { Button } from "./kit.tsx";

/** What a person does next, by the kind of failure; a bare message is a passing failure. */
function nextStep(kind: ReadError["kind"]): string {
  switch (kind) {
    case "denied":
      return "Your sign-in may have ended: reload the page to sign in again.";
    case "skew":
      return "This page is out of date: reload it.";
    case "refused":
      return "Cairn could not make sense of what it got.";
    case "transient":
      return "Try again in a moment.";
  }
}

/** "`what` could not be read", the next step, and the raw message on request. */
export function ReadFailed({ what, error }: { what: string; error: ReadError | string }) {
  const [shown, setShown] = useState(false);
  const { kind, message } = typeof error === "string" ? { kind: "transient" as const, message: error } : error;
  return (
    <>
      {what} could not be read. {nextStep(kind)}{" "}
      <Button ghost className="small" aria-expanded={shown} onClick={() => { setShown(!shown); }}>
        Details
      </Button>
      {shown ? <code className="read-failed-message" data-testid="read-failed-message"> {message}</code> : null}
    </>
  );
}
