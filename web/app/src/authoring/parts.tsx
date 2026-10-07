// The pieces every structure editor is built from: a field with its label, the problem that
// stops it being sent (a limit, PRACTICES), and the violations the engine reports about it
// (A15: each at its field); a select over options; and a rejection that no field owns.
import type { ReactNode, SelectHTMLAttributes } from "react";

import type { Rejection } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { Button } from "../ui/kit.tsx";
import type { Place, Violation } from "./violations.ts";

/** What a field shows beside its control. */
export interface FieldNotes {
  /** Why the value cannot be sent, before sending. */
  problem?: string | undefined;
  /** What the engine said about it. */
  violations?: readonly Violation[] | undefined;
  /** B4: the field is edited here, away from the route. */
  edited?: boolean | undefined;
}

/** One field: its label, its control, and what stops or rejects it. */
export function FieldBox({ label, place, notes = {}, children }: { label: string; place: string; notes?: FieldNotes | undefined; children: ReactNode }) {
  const violations = notes.violations ?? [];
  const invalid = notes.problem !== undefined || violations.length > 0;
  return (
    <div className="stack author-field" data-testid="author-field" data-field={place} data-invalid={invalid ? "true" : "false"}>
      <span className="row">
        {label === "" ? null : <span className="author-label">{label}</span>}
        {notes.edited === true ? (
          <span className="badge badge-warn" data-testid="edited-here">
            edited here
          </span>
        ) : null}
      </span>
      {children}
      {notes.problem === undefined ? null : (
        <span className="author-problem" role="alert" data-testid="problem">
          {notes.problem}
        </span>
      )}
      {violations.map((violation, at) => (
        <span key={at} className="author-problem" role="alert" data-testid="violation" data-code={violation.code}>
          {violation.message}
        </span>
      ))}
    </div>
  );
}

/** A select over `options` (value and label), with an optional "none" first. */
export function Picker({
  options,
  none,
  ...props
}: Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> & { options: readonly { value: string; label: string }[]; none?: string }) {
  return (
    <select className="select" {...props}>
      {none === undefined ? null : <option value="">{none}</option>}
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

/** The violations a rejection lists that no field of the form owns. */
export function FormViolations({ places }: { places: ReadonlyMap<Place, Violation[]> }) {
  const loose = places.get("form") ?? [];
  if (loose.length === 0) {
    return null;
  }
  return (
    <ul className="detail-list callout callout-bad" role="alert" data-testid="form-violations">
      {loose.map((violation, at) => (
        <li key={at} data-testid="violation" data-code={violation.code}>
          {violation.message}
        </li>
      ))}
    </ul>
  );
}

/** A write's rejection that is not about fields: a stale base (H5), shown with a retry on the current version. */
export function StaleRejection({ rejection, onRetry, onDismiss }: { rejection: Rejection; onRetry: () => void; onDismiss: () => void }) {
  if (rejection.rejection === "invalid") {
    return null;
  }
  return (
    <div className="stack">
      <RejectionView rejection={rejection} onRebase={onRetry} />
      <span className="row">
        <Button onClick={onDismiss}>Drop this change</Button>
      </span>
    </div>
  );
}
