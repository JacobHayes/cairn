// C9's filters: the flags (mine, unassigned, next up, decisions needed, needs breakdown,
// active, blocked, overdue, stale, snoozed, snoozed and overdue, shortfall), by group (a
// container's subtree), by owner, by state, by kind; text search over title, description,
// notes, and resources; the sort; and grouping by container. Every filter holds at once.
import { useState } from "react";

import { entityName } from "../detail/sections.tsx";
import type { Ready } from "../detail/model.ts";
import { Field } from "../ui/kit.tsx";
import { LIST_FLAGS, LIST_KINDS, STATES, type ListFlag, type ListSettings } from "./address.ts";
import { Check, Checks, SortSelect } from "./Controls.tsx";

const FLAG_WORDS: Record<ListFlag, string> = {
  mine: "Mine",
  unassigned: "Unassigned",
  next_up: "Next up",
  decisions_needed: "Decisions needed",
  needs_breakdown: "Needs breakdown",
  active: "Active",
  blocked: "Blocked",
  overdue: "Overdue",
  stale: "Stale",
  snoozed: "Snoozed",
  snoozed_and_overdue: "Snoozed and overdue",
  shortfall: "Shortfall",
};

/** The journey's containers (every node with children), each by its path of titles, in tree order. */
function containersOf(view: Ready): { key: string; path: string }[] {
  const nodes = view.journey.graph.nodes ?? [];
  const parents = new Set(nodes.map((node) => node.parent).filter((parent) => parent != null));
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const pathOf = (key: string): string => {
    const titles: string[] = [];
    let at = byKey.get(key);
    while (at !== undefined && titles.length <= nodes.length) {
      titles.unshift(at.title);
      at = at.parent == null ? undefined : byKey.get(at.parent);
    }
    return titles.join(" / ");
  };
  return nodes
    .filter((node) => parents.has(node.key))
    .map((node) => ({ key: node.key, path: pathOf(node.key) }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

function Search({ text, onChange }: { text: string; onChange: (text: string) => void }) {
  const [typed, setTyped] = useState(text);
  return (
    <form
      className="row"
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        onChange(typed);
      }}
    >
      <Field type="search" aria-label="Search" placeholder="Search titles, descriptions, notes, resources" value={typed} onChange={(event) => { setTyped(event.target.value); }} />
      <button type="submit" className="button">
        Search
      </button>
    </form>
  );
}

function Pick({ label, value, options, onChange }: { label: string; value: string | undefined; options: { key: string; text: string }[]; onChange: (value: string | undefined) => void }) {
  return (
    <select aria-label={label} value={value ?? ""} onChange={(event) => { onChange(event.target.value === "" ? undefined : event.target.value); }}>
      <option value="">{label}: any</option>
      {options.map((option) => (
        <option key={option.key} value={option.key}>
          {option.text}
        </option>
      ))}
    </select>
  );
}

export function ListFilters({ view, settings, onChange }: { view: Ready; settings: ListSettings; onChange: (next: ListSettings) => void }) {
  const set = (changed: Partial<ListSettings>) => {
    onChange({ ...settings, ...changed });
  };
  const owners = (view.inputs.deployment.entities ?? []).map((entity) => ({ key: entity.key, text: entityName(view, entity.key) }));
  return (
    <div className="stack acting-controls" data-testid="list-filters">
      <div className="row">
        <Search key={settings.text} text={settings.text} onChange={(text) => { set({ text }); }} />
        <SortSelect sort={settings.sort} onChange={(sort) => { set({ sort }); }} />
        <Check label="Group by container" checked={settings.grouped} testId="grouped" onChange={(grouped) => { set({ grouped }); }} />
      </div>
      <Checks legend="Show only" options={LIST_FLAGS} chosen={settings.flags} words={(flag) => FLAG_WORDS[flag]} testId="flag" onChange={(flags) => { set({ flags }); }} />
      <div className="row">
        <Pick label="Group" value={settings.within} options={containersOf(view).map(({ key, path }) => ({ key, text: path }))} onChange={(within) => { set({ within }); }} />
        <Pick label="Owner" value={settings.owner} options={owners} onChange={(owner) => { set({ owner }); }} />
      </div>
      <Checks legend="State" options={STATES} chosen={settings.states} words={(state) => state} testId="state" onChange={(states) => { set({ states }); }} />
      <Checks legend="Kind" options={LIST_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" onChange={(kinds) => { set({ kinds }); }} />
    </div>
  );
}
