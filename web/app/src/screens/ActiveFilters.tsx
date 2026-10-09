// The active filters as removable chips under the toolbar (`overdue ×`, 2.3). The row is
// absent when no filter is active. The chips are the toolbar's filters in words: each one
// takes its own setting off and leaves the rest as they were (filters.ts).
import { Link } from "react-router";

import type { ActiveFilter } from "./filters.ts";

export function ActiveFilters({ filters }: { filters: ActiveFilter[] }) {
  if (filters.length === 0) {
    return null;
  }
  return (
    <div className="active-filters row" role="group" aria-label="Active filters" data-testid="active-filters">
      {filters.map((filter) => (
        <Link key={filter.id} to={filter.without} className="chip chip-on" data-testid="active-filter" data-filter={filter.id} aria-label={`Remove the filter ${filter.label}`}>
          {filter.label} <span aria-hidden="true">×</span>
        </Link>
      ))}
    </div>
  );
}
