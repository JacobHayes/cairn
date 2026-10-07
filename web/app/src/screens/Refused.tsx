// A refused write on the screens around a journey, shown whole (A15): a stale one with what
// intervened (H5), an invalid one with each violation's sentence and code, and a reused
// patch id. `children` is what the screen offers against it (A11: discard the existing draft).
import type { ReactNode } from "react";

import type { Rejection } from "../data/writes.ts";
import { Button } from "../ui/kit.tsx";
import { describeRecord } from "./RejectionView.tsx";

export function Refused({ rejection, onDismiss, children }: { rejection: Rejection; onDismiss: () => void; children?: ReactNode }) {
  let body: ReactNode;
  if (rejection.rejection === "stale") {
    const records = [...new Set(rejection.intervening.map(describeRecord))];
    body = <span>Someone changed this since you opened it ({records.join(", ")}). Try again on the current version.</span>;
  } else if (rejection.rejection === "invalid") {
    body = (
      <ul className="stack">
        {rejection.violations.map((violation, at) => (
          <li key={at} data-testid="violation" data-code={violation.code}>
            {violation.message} <span className="muted mono">{violation.code}</span>
          </li>
        ))}
      </ul>
    );
  } else {
    body = <span>This patch id was already used for another change ({rejection.patch_id}).</span>;
  }
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="refused" data-rejection={rejection.rejection}>
      <strong>Not saved.</strong>
      {body}
      <span className="row">
        {children}
        <Button onClick={onDismiss}>Dismiss</Button>
      </span>
    </div>
  );
}

/** Whether `rejection` is invalid with a violation of `code`. */
export function violates(rejection: Rejection | undefined, code: string): boolean {
  return rejection?.rejection === "invalid" && rejection.violations.some((violation) => violation.code === code);
}
