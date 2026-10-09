// The controls the acting surfaces share: the next list's sort (C10), a set of checkboxes, a
// single checkbox, and the flags a filter offers.
import type { ReactNode } from "react";

import type { ListFlag, SortBy } from "./address.ts";

/** The sort menu's choices: each signal, and the ranking for the viewer (Priority: rank for me). */
type SortChoice = SortBy | "me";

const SORT_CHOICES: { value: SortChoice; words: string }[] = [
  { value: "rank", words: "Rank" },
  { value: "me", words: "Rank for me" },
  { value: "due", words: "Due" },
  { value: "slack", words: "Start by" },
  { value: "gravity", words: "Gravity" },
  { value: "leverage", words: "Unblocks" },
  { value: "effort", words: "Effort" },
];

/** C9, C10: the sort, in the list's header: one signal, or (given `forMe`) the rank for the viewer. */
export function SortSelect({ sort, forMe, onChange }: { sort: SortBy; forMe?: boolean; onChange: (sort: SortBy, forMe: boolean) => void }) {
  return (
    <label className="row next-sort">
      <span className="muted small">Sort</span>
      <select
        aria-label="Sort by"
        value={forMe === true && sort === "rank" ? "me" : sort}
        onChange={(event) => {
          const chosen = SORT_CHOICES.find((each) => each.value === event.target.value)?.value ?? "rank";
          onChange(chosen === "me" ? "rank" : chosen, chosen === "me");
        }}
      >
        {SORT_CHOICES.filter((each) => each.value !== "me" || forMe !== undefined).map((each) => (
          <option key={each.value} value={each.value}>
            {each.words}
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
