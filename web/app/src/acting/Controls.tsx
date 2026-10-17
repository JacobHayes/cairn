// The controls the toolbar's popovers and the acting surfaces share: a set of checkboxes, a
// single checkbox, and the flags a filter offers.
import type { ReactNode } from "react";

import { FLAG_WORDS } from "../journeys/view.ts";
import type { ListFlag } from "./address.ts";

/** One checkbox; `disabled` where the view cannot honour it, keeping the value it carries (DESIGN, disabled controls). */
export function Check({ label, checked, onChange, testId, disabled = false }: { label: ReactNode; checked: boolean; onChange: (checked: boolean) => void; testId: string; disabled?: boolean }) {
  return (
    <label className="check" data-testid={testId} data-status={checked ? "on" : "off"}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => { onChange(event.target.checked); }} /> {label}
    </label>
  );
}

/** Checkboxes over `options`, `chosen` checked; the change is the chosen set in `options`' order. */
export function Checks<T extends string>({
  legend,
  options,
  chosen,
  words,
  onChange,
  testId,
  stacked = false,
}: {
  legend: string;
  options: readonly T[];
  chosen: readonly T[];
  words: (option: T) => string;
  onChange: (chosen: T[]) => void;
  testId: string;
  /** One option to a line, under its legend. */
  stacked?: boolean;
}) {
  return (
    <fieldset className={stacked ? "checks checks-stacked stack" : "checks row"} data-testid={testId}>
      <legend className="muted small">{legend}</legend>
      {options.map((option) => (
        <Check
          key={option}
          label={words(option)}
          checked={chosen.includes(option)}
          testId={`${testId}-${option}`}
          onChange={(checked) => {
            onChange(options.filter((each) => (each === option ? checked : chosen.includes(each))));
          }}
        />
      ))}
    </fieldset>
  );
}

/** The flags a filter offers, one to a line; the change is the chosen set in `options`' order. */
export function FlagChecks({ options, chosen, onChange }: { options: readonly ListFlag[]; chosen: readonly ListFlag[]; onChange: (chosen: ListFlag[]) => void }) {
  return <Checks legend="Flags" options={options} chosen={chosen} words={(flag) => FLAG_WORDS[flag] ?? flag} testId="flag" stacked onChange={onChange} />;
}
