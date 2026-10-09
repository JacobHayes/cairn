// C9's filters, as the toolbar's Filter holds them: mine, the kinds, the flags (overdue, stale,
// unassigned, short of days, snoozed) and the owner; text search is the toolbar's, and the sort
// and the grouping by container are in the list's header (ListScreen). Every filter holds at
// once. An address can still carry the filters the popover no longer offers (a group, a state,
// the other flags): they read as before and show as chips that remove them.
import { entityName } from "../detail/sections.tsx";
import type { Ready } from "../detail/model.ts";
import { LIST_FILTER_FLAGS, LIST_FLAGS, LIST_KINDS, type ListFlag, type ListSettings } from "./address.ts";
import { Check, Checks, FlagChecks } from "./Controls.tsx";

/** `flags` with those among `offered` replaced by `chosen`: the ones the popover does not offer stay as the address has them. */
function withChosen(flags: readonly ListFlag[], offered: readonly ListFlag[], chosen: readonly ListFlag[]): ListFlag[] {
  return LIST_FLAGS.filter((flag) => (offered.includes(flag) ? chosen.includes(flag) : flags.includes(flag)));
}

export function ListFilters({ view, settings, onChange }: { view: Ready; settings: ListSettings; onChange: (next: ListSettings) => void }) {
  const set = (changed: Partial<ListSettings>) => {
    onChange({ ...settings, ...changed });
  };
  const owners = (view.inputs.deployment.entities ?? []).map((entity) => ({ key: entity.key, text: entityName(view, entity.key) }));
  return (
    <div className="stack acting-controls" data-testid="list-filters">
      <Check label="Only mine" checked={settings.flags.includes("mine")} testId="flag-mine" onChange={(mine) => { set({ flags: withChosen(settings.flags, ["mine"], mine ? ["mine"] : []) }); }} />
      <Checks legend="Kinds" options={LIST_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" stacked onChange={(kinds) => { set({ kinds }); }} />
      <FlagChecks options={LIST_FILTER_FLAGS} chosen={settings.flags} onChange={(chosen) => { set({ flags: withChosen(settings.flags, LIST_FILTER_FLAGS, chosen) }); }} />
      <select
        aria-label="Owner"
        value={settings.owner ?? ""}
        onChange={(event) => { set({ owner: event.target.value === "" ? undefined : event.target.value }); }}
      >
        <option value="">Owner: any</option>
        {owners.map((owner) => (
          <option key={owner.key} value={owner.key}>
            {owner.text}
          </option>
        ))}
      </select>
    </div>
  );
}
