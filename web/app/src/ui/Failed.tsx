// A read or a write that failed, said to a person: one sentence for the kind of failure and a
// next step (never the server's own message, which may name internal ids), the action that
// leads (Try again, passed as children), and the raw message behind a quiet Details button for
// whoever needs to report it. e2e/internal-ids.ts fails a screen that shows an id.
import { useState, type ReactNode } from "react";

import type { HttpFailure } from "@cairn/client";

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

/** A failed HTTP answer as the kind of failure it is: a refusal of the sign-in, else a passing failure. */
export function kindOf(failure: HttpFailure): ReadError {
  return { kind: failure.status === 401 || failure.status === 403 ? "denied" : "transient", message: failure.message };
}

/**
 * "`what` could not be `verb`" (or `said`, when the sentence is already written), the next step,
 * `children` (the action that leads), and the raw message on request. `retry` says a Try again button stands
 * beside the sentence, so a passing failure is not also told to try again in words.
 */
export function Failed({ what, verb = "read", said, error, retry = false, children }: { what?: string; verb?: string; said?: string; error: ReadError | string; retry?: boolean; children?: ReactNode }) {
  const [shown, setShown] = useState(false);
  const { kind, message } = typeof error === "string" ? { kind: "transient" as const, message: error } : error;
  return (
    <>
      {said ?? `${what ?? "That"} could not be ${verb}.`} {retry && kind === "transient" ? null : nextStep(kind)}{" "}
      {children === undefined ? null : <>{children}{" "}</>}
      <Button ghost className="failed-details" aria-expanded={shown} onClick={() => { setShown(!shown); }}>
        Details
      </Button>
      {shown ? <code className="failed-message" data-testid="failed-message"> {message}</code> : null}
    </>
  );
}

/** A write's problem: its sentence, with the server's own message behind Details when it has one. */
export function WriteProblem({ message, detail }: { message: string; detail: HttpFailure | undefined }) {
  return detail === undefined ? message : <Failed said={message} error={kindOf(detail)} />;
}
