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

/** A label with the count of what its setting hides beside it, in neutral type; the label alone when it hides none. */
export function Counted({ label, count, testId }: { label: ReactNode; count: number | undefined; testId: string }) {
  return (
    <>
      {label}
      {count === undefined ? null : (
        <>
          {" "}
          <span className="muted small check-count" data-testid={`${testId}-count`}>
            {count}
          </span>
        </>
      )}
    </>
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
  counts = {},
}: {
  legend: string;
  options: readonly T[];
  chosen: readonly T[];
  words: (option: T) => string;
  onChange: (chosen: T[]) => void;
  testId: string;
  /** One option to a line, under its legend. */
  stacked?: boolean;
  /** What each option hides, beside it. */
  counts?: Partial<Record<T, number>>;
}) {
  return (
    <fieldset className={stacked ? "checks checks-stacked stack" : "checks row"} data-testid={testId}>
      <legend className="label">{legend}</legend>
      {options.map((option) => (
        <Check
          key={option}
          label={<Counted label={words(option)} count={counts[option]} testId={`${testId}-${option}`} />}
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
export function FlagChecks({ options, chosen, counts = {}, onChange }: { options: readonly ListFlag[]; chosen: readonly ListFlag[]; counts?: Partial<Record<ListFlag, number>>; onChange: (chosen: ListFlag[]) => void }) {
  return <Checks legend="Flags" options={options} chosen={chosen} words={(flag) => FLAG_WORDS[flag] ?? flag} testId="flag" stacked counts={counts} onChange={onChange} />;
}
