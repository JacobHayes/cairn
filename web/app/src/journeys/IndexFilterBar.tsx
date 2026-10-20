// C16's filters over the journey index: status, lineage route and version, "mine", and
// "upgrade available". Every filter holds at once; each change is a new address.
import { Check } from "../acting/Controls.tsx";
import type { RouteSummary } from "../data/reads.ts";
import { FilterSelect } from "../ui/IndexHead.tsx";
import { STATUSES, type IndexFilters } from "./address.ts";

const STATUS_WORDS = { active: "Active", completed: "Completed", archived: "Archived", any: "Any status" } as const;

/** The journey index's filters, as the toolbar's controls: every one holds at once; each change is a new address. */
export function IndexFilterBar({ filters, routes, onChange }: { filters: IndexFilters; routes: readonly RouteSummary[]; onChange: (filters: IndexFilters) => void }) {
  const route = routes.find((each) => each.header.id === filters.route);
  const versions = Array.from({ length: route?.latest_version ?? 0 }, (_, at) => at + 1);
  return (
    <>
      <FilterSelect
        label="Status"
        value={filters.status}
        options={[...STATUSES, "any" as const].map((status) => ({ value: status, label: STATUS_WORDS[status] }))}
        onChange={(value) => { onChange({ ...filters, status: [...STATUSES, "any" as const].find((each) => each === value) ?? "active" }); }}
      />
      <FilterSelect
        label="Route"
        value={filters.route ?? ""}
        options={[{ value: "", label: "Any route" }, ...routes.map((each) => ({ value: each.header.id, label: each.header.name }))]}
        onChange={(value) => { onChange({ ...filters, route: value === "" ? undefined : value, version: undefined }); }}
      />
      {filters.route === undefined ? null : (
        <FilterSelect
          label="Version"
          value={filters.version === undefined ? "" : String(filters.version)}
          options={[{ value: "", label: "Any version" }, ...versions.map((version) => ({ value: String(version), label: String(version) }))]}
          onChange={(value) => { onChange({ ...filters, version: value === "" ? undefined : Number(value) }); }}
        />
      )}
      <Check label="Mine" checked={filters.mine} testId="filter-mine" onChange={(mine) => { onChange({ ...filters, mine }); }} />
      <Check label="Upgrade available" checked={filters.upgrade} testId="filter-upgrade" onChange={(upgrade) => { onChange({ ...filters, upgrade }); }} />
    </>
  );
}
