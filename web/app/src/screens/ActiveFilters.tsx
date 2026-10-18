// The Filter's settings as removable chips under the toolbar (`overdue ×`, 2.3).
// The row is absent when nothing is on. Each chip takes its own setting off and leaves the rest
// as they were (journeys/view.ts, `filterChips`).
import { Link } from "react-router";

import { Chip, Cross } from "../ui/Chip.tsx";

/** One chip: its words, and the address with its setting off (the open node stays open). */
export interface ActiveFilter {
  id: string;
  label: string;
  without: string;
}

export function ActiveFilters({ filters }: { filters: ActiveFilter[] }) {
  if (filters.length === 0) {
    return null;
  }
  return (
    <div className="active-filters row" role="group" aria-label="Active filters" data-testid="active-filters">
      {filters.map((filter) => (
        <Chip
          key={filter.id}
          remove={
            <Link to={filter.without} data-testid="active-filter" data-filter={filter.id} aria-label={`Remove the filter ${filter.label}`}>
              <Cross />
            </Link>
          }
        >
          {filter.label}
        </Chip>
      ))}
    </div>
  );
}
