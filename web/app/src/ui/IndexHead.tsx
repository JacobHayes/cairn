// The head every index page shares (Journeys, Routes, Identities): the title with the page's
// primary action (New journey, New route, Add) at its right end, and under it one toolbar row of filters and tools. The
// action and the toolbar sit in the same place on every index.
import type { ReactNode } from "react";

/** A labelled select for a toolbar: a filter over the rows beneath. */
export function FilterSelect({ label, value, options, onChange }: { label: string; value: string; options: readonly { value: string; label: string }[]; onChange: (value: string) => void }) {
  return (
    <label className="row">
      <span className="filter-label">{label}</span>
      <select aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/** A search box for a toolbar: a filter over the rows beneath, labelled like the selects beside it. */
export function FilterSearch({ label, placeholder, value, onChange }: { label: string; placeholder: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className="row">
      <span className="filter-label">{label}</span>
      <input type="search" aria-label={label} placeholder={placeholder} value={value} onChange={(event) => { onChange(event.target.value); }} />
    </label>
  );
}

export function IndexHead({ title, action, children }: { title: string; action?: ReactNode; children?: ReactNode }) {
  return (
    <>
      <div className="row index-head">
        <h1>{title}</h1>
        <span className="spacer" />
        {action}
      </div>
      {children === undefined ? null : (
        <div className="row index-filters" data-testid="index-filters">
          {children}
        </div>
      )}
    </>
  );
}
