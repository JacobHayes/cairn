// C16's filters over the journey index: status, lineage route and version, owner, "mine", and
// "upgrade available". Every filter holds at once; each change is a new address.
import { JOURNEY_STATUSES } from "@cairn/client";

import { Check } from "../acting/Controls.tsx";
import { useCaller, useDeployment } from "../data/react.ts";
import type { RouteSummary } from "../data/reads.ts";
import { identityLabel } from "../people/label.ts";
import { FilterSelect } from "../ui/IndexHead.tsx";
import type { IndexFilters } from "./address.ts";
import { STATUS_WORDS } from "./StatusBadge.tsx";

/** The owners the filter offers: the viewer's own identity as "You", then everyone else by name. */
function ownerOptions(identities: readonly { key: string; name?: string | null | undefined; emails?: readonly string[] | null | undefined }[], viewer: readonly string[]) {
  const [own] = viewer;
  const others = identities
    .filter((identity) => identity.key !== own)
    .map((identity) => ({ value: identity.key, label: identityLabel(identity) }))
    .sort((left, right) => left.label.localeCompare(right.label));
  return [{ value: "", label: "Any owner" }, ...(own === undefined ? [] : [{ value: own, label: "You" }]), ...others];
}

/** The journey index's filters, as the toolbar's controls: every one holds at once; each change is a new address. */
export function IndexFilterBar({ filters, routes, onChange }: { filters: IndexFilters; routes: readonly RouteSummary[]; onChange: (filters: IndexFilters) => void }) {
  const route = routes.find((each) => each.header.id === filters.route);
  const identities = useDeployment()?.identities ?? [];
  const { caller } = useCaller();
  const versions = Array.from({ length: route?.latest_version ?? 0 }, (_, at) => at + 1);
  return (
    <>
      <FilterSelect
        label="Status"
        value={filters.status}
        options={[...JOURNEY_STATUSES, "any" as const].map((status) => ({ value: status, label: status === "any" ? "Any status" : STATUS_WORDS[status] }))}
        onChange={(value) => { onChange({ ...filters, status: [...JOURNEY_STATUSES, "any" as const].find((each) => each === value) ?? "active" }); }}
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
      {identities.length === 0 ? null : (
        <FilterSelect
          label="Owner"
          value={filters.owner ?? ""}
          options={ownerOptions(identities, caller?.identities ?? [])}
          onChange={(value) => { onChange({ ...filters, owner: value === "" ? undefined : value }); }}
        />
      )}
      <Check label="Mine" checked={filters.mine} testId="filter-mine" onChange={(mine) => { onChange({ ...filters, mine }); }} />
      <Check label="Upgrade available" checked={filters.upgrade} testId="filter-upgrade" onChange={(upgrade) => { onChange({ ...filters, upgrade }); }} />
    </>
  );
}
