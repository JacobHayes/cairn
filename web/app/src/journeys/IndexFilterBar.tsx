// C16's filters over the journey index: status, lineage route and version, "mine", and
// "upgrade available". Every filter holds at once; each change is a new address.
import { Check } from "../acting/Controls.tsx";
import type { RouteSummary } from "../data/reads.ts";
import { STATUSES, type IndexFilters } from "./address.ts";

const STATUS_WORDS = { active: "Active", completed: "Completed", archived: "Archived", any: "Any status" } as const;

function RouteFilter({ filters, routes, onChange }: { filters: IndexFilters; routes: readonly RouteSummary[]; onChange: (filters: IndexFilters) => void }) {
  const route = routes.find((each) => each.header.id === filters.route);
  const versions = Array.from({ length: route?.latest_version ?? 0 }, (_, at) => at + 1);
  return (
    <>
      <label className="row">
        <span className="muted">Route</span>
        <select
          className="select"
          aria-label="Route"
          value={filters.route ?? ""}
          onChange={(event) => { onChange({ ...filters, route: event.target.value === "" ? undefined : event.target.value, version: undefined }); }}
        >
          <option value="">Any route</option>
          {routes.map((each) => (
            <option key={each.header.id} value={each.header.id}>
              {each.header.name}
            </option>
          ))}
        </select>
      </label>
      {filters.route === undefined ? null : (
        <label className="row">
          <span className="muted">Version</span>
          <select
            className="select"
            aria-label="Version"
            value={filters.version ?? ""}
            onChange={(event) => { onChange({ ...filters, version: event.target.value === "" ? undefined : Number(event.target.value) }); }}
          >
            <option value="">Any version</option>
            {versions.map((version) => (
              <option key={version} value={version}>
                {version}
              </option>
            ))}
          </select>
        </label>
      )}
    </>
  );
}

export function IndexFilterBar({ filters, routes, onChange }: { filters: IndexFilters; routes: readonly RouteSummary[]; onChange: (filters: IndexFilters) => void }) {
  return (
    <div className="row acting-controls" data-testid="index-filters">
      <label className="row">
        <span className="muted">Status</span>
        <select
          className="select"
          aria-label="Status"
          value={filters.status}
          onChange={(event) => { onChange({ ...filters, status: [...STATUSES, "any" as const].find((each) => each === event.target.value) ?? "active" }); }}
        >
          {[...STATUSES, "any" as const].map((status) => (
            <option key={status} value={status}>
              {STATUS_WORDS[status]}
            </option>
          ))}
        </select>
      </label>
      <RouteFilter filters={filters} routes={routes} onChange={onChange} />
      <Check label="Mine" checked={filters.mine} testId="filter-mine" onChange={(mine) => { onChange({ ...filters, mine }); }} />
      <Check label="Upgrade available" checked={filters.upgrade} testId="filter-upgrade" onChange={(upgrade) => { onChange({ ...filters, upgrade }); }} />
    </div>
  );
}
