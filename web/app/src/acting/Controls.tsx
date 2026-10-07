// The controls the acting surfaces share: a sort by one signal (C9, C10: so the trade-off rank
// blends is visible), a set of checkboxes, and a single checkbox.
import type { ReactNode } from "react";

import { SORTS, type SortBy } from "./address.ts";

const SORT_WORDS: Record<SortBy, string> = {
  rank: "Rank",
  slack: "Slack (least first)",
  gravity: "Gravity",
  leverage: "Leverage",
  due: "Due (soonest first)",
  effort: "Gravity per day of effort",
};

export function SortSelect({ sort, onChange }: { sort: SortBy; onChange: (sort: SortBy) => void }) {
  return (
    <label className="row">
      <span className="muted">Sort by</span>
      <select
        className="select"
        aria-label="Sort by"
        value={sort}
        onChange={(event) => { onChange(SORTS.find((each) => each === event.target.value) ?? "rank"); }}
      >
        {SORTS.map((each) => (
          <option key={each} value={each}>
            {SORT_WORDS[each]}
          </option>
        ))}
      </select>
    </label>
  );
}

export function Check({ label, checked, onChange, testId }: { label: ReactNode; checked: boolean; onChange: (checked: boolean) => void; testId: string }) {
  return (
    <label className="check" data-testid={testId} data-status={checked ? "on" : "off"}>
      <input type="checkbox" checked={checked} onChange={(event) => { onChange(event.target.checked); }} /> {label}
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
}: {
  legend: string;
  options: readonly T[];
  chosen: readonly T[];
  words: (option: T) => string;
  onChange: (chosen: T[]) => void;
  testId: string;
}) {
  return (
    <fieldset className="checks row" data-testid={testId}>
      <legend className="muted">{legend}</legend>
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
