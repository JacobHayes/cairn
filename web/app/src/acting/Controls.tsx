// The controls the acting surfaces share: a sort by one signal (C9, C10: so the trade-off rank
// blends is visible), a set of checkboxes, a single checkbox, and the flags a filter offers.
import type { ReactNode } from "react";

import { SORTS, type ListFlag, type SortBy } from "./address.ts";

const FOR_ME = "rank-for-me";

const SORT_WORDS: Record<SortBy, string> = {
  rank: "Rank",
  slack: "Slack (least first)",
  gravity: "Gravity",
  leverage: "Leverage",
  due: "Due (soonest first)",
  effort: "Gravity per day of effort",
};

/**
 * The sort by one signal. Given `forMe` (NEXT), "Rank for me" follows "Rank" in the options: it
 * is the rank with the owner factor relative to the viewer, a way of ranking and not a filter.
 */
export function SortSelect({ sort, forMe, onChange }: { sort: SortBy; forMe?: boolean; onChange: (sort: SortBy, forMe: boolean) => void }) {
  const value = forMe === true && sort === "rank" ? FOR_ME : sort;
  return (
    <label className="row">
      <span className="muted small">Sort by</span>
      <select
        aria-label="Sort by"
        value={value}
        onChange={(event) => {
          const chosen = event.target.value;
          onChange(chosen === FOR_ME ? "rank" : (SORTS.find((each) => each === chosen) ?? "rank"), chosen === FOR_ME);
        }}
      >
        {SORTS.flatMap((each) => [
          <option key={each} value={each}>
            {SORT_WORDS[each]}
          </option>,
          ...(each === "rank" && forMe !== undefined ? [<option key={FOR_ME} value={FOR_ME}>Rank for me</option>] : []),
        ])}
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

const FLAG_WORDS: Partial<Record<ListFlag, string>> = { overdue: "Overdue", stale: "Stale", unassigned: "Unassigned", shortfall: "Short of days", snoozed: "Snoozed" };

/** The flags a filter offers, one to a line; the change is the chosen set in `options`' order. */
export function FlagChecks({ options, chosen, onChange }: { options: readonly ListFlag[]; chosen: readonly ListFlag[]; onChange: (chosen: ListFlag[]) => void }) {
  return <Checks legend="Flags" options={options} chosen={chosen} words={(flag) => FLAG_WORDS[flag] ?? flag} testId="flag" stacked onChange={onChange} />;
}
