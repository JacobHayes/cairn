// The pieces every structure editor is built from: a field with its label, the problem that
// stops it being sent (a limit, PRACTICES), and the violations the engine reports about it
// (A15: each at its field); a select over options; and a rejection that no field owns.
import { useId, useLayoutEffect, useRef, type ReactNode, type SelectHTMLAttributes } from "react";

import type { ReadError } from "../data/keyed-read.ts";
import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";

import type { Violation } from "../data/writes.ts";
import { ViolationLines } from "../screens/RejectionView.tsx";
import type { Place } from "./violations.ts";

/** What a field shows beside its control. */
export interface FieldNotes {
  /** Why the value cannot be sent, before sending. */
  problem?: string | undefined;
  /** What the engine said about it. */
  violations?: readonly Violation[] | undefined;
  /** B4: the field is edited here, away from the route. */
  edited?: boolean | undefined;
}

/** The attributes of a control whose value has a problem: it is invalid, and points to the message `id` names. */
export function invalidAttrs(id: string, problem: string | undefined): { "aria-invalid"?: true; "aria-describedby"?: string } {
  return problem === undefined ? {} : { "aria-invalid": true, "aria-describedby": id };
}

/** The message of a problem that stops a value being sent, with the `id` its control points to. */
export function Problem({ id, children }: { id: string; children: ReactNode }) {
  return (
    <span id={id} className="author-problem" role="alert" data-testid="problem">
      {children}
    </span>
  );
}

/** Why a check of the form's values could not be made, with a way to ask again. */
export function CheckFailed({ failure, retry }: { failure: ReadError | undefined; retry: () => void }) {
  if (failure === undefined) {
    return null;
  }
  return (
    <span className="author-problem" role="alert" data-testid="check-failed">
      <Failed said="The values could not be checked, so they cannot be saved yet." error={failure} retry>
        <Button onClick={retry}>Try again</Button>
      </Failed>
    </span>
  );
}

/** One field: its label, its control, and what stops or rejects it. */
export function FieldBox({ label, place, notes = {}, children }: { label: string; place: string; notes?: FieldNotes | undefined; children: ReactNode }) {
  const violations = notes.violations ?? [];
  const invalid = notes.problem !== undefined || violations.length > 0;
  const message = useId();
  const controls = useRef<HTMLDivElement>(null);
  // The controls are the editors' own (a text box, or a row of them): each of this field's, and not of a field inside it, is marked invalid and points to the message.
  useLayoutEffect(() => {
    for (const control of controls.current?.querySelectorAll("input, select, textarea") ?? []) {
      if (control.closest("[data-field]") !== controls.current) {
        continue;
      }
      if (invalid) {
        control.setAttribute("aria-invalid", "true");
      } else {
        control.removeAttribute("aria-invalid");
      }
      if (notes.problem === undefined) {
        control.removeAttribute("aria-describedby");
      } else {
        control.setAttribute("aria-describedby", message);
      }
    }
  });
  return (
    <div ref={controls} className="stack author-field" data-testid="author-field" data-field={place} data-invalid={invalid ? "true" : "false"}>
      <span className="row">
        {label === "" ? null : <span className="label">{label}</span>}
        {notes.edited === true ? (
          <span className="badge warn" data-testid="edited-here">
            edited here
          </span>
        ) : null}
      </span>
      {children}
      {notes.problem === undefined ? null : <Problem id={message}>{notes.problem}</Problem>}
      <ViolationLines violations={violations} />
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
    <select {...props}>
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
    <div className="stack callout callout-bad" data-testid="form-violations">
      <ViolationLines violations={loose} />
    </div>
  );
}
