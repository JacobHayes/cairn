// C9: the list. The journey flattened to a table with every filter, grouping by container,
// sort by one signal, text search, and multi-select for bulk actions (BulkBar.tsx). The
// engine's `list` projection answers it from the tab's derivation a page at a time; what it
// shows lives in the address. It is PLAN, LIST: the journey page (screens/JourneyFrame.tsx)
// holds its toolbar and the filters it opens (ListFilters); the sort and the grouping are in the
// list's header.
import { Fragment, useMemo, useState } from "react";

import { useProjected } from "../canvas/hooks.ts";
import type { Ready } from "../detail/model.ts";
import { entityName } from "../detail/sections.tsx";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { selectionOf } from "./acts.ts";
import { listQueryOf, type ListSettings } from "./address.ts";
import { BulkBar } from "./BulkBar.tsx";
import { Check, SortSelect } from "./Controls.tsx";
import { Crumb, DetailLink, Flags } from "./Parts.tsx";
import { byContainer } from "./rows.ts";
import type { NodeRow } from "./why.ts";
import "./acting.css";

const COLUMNS = ["Node", "Kind", "State", "Owner", "Due", "Slack", "Gravity", "Leverage", "Rank", "Flags"];

function Row({ view, row, grouped, chosen, onToggle }: { view: Ready; row: NodeRow; grouped: boolean; chosen: boolean; onToggle: () => void }) {
  return (
    <tr data-testid="list-row" data-node={row.key} data-slack={row.slack_days ?? ""} data-selected={chosen}>
      <td>
        <input type="checkbox" aria-label={`Select ${row.title}`} checked={chosen} onChange={onToggle} />
      </td>
      <td>
        <div className="list-node">
          <DetailLink view={view} node={row.key} />
          {grouped ? null : <Crumb view={view} row={row} />}
        </div>
      </td>
      <td>{row.kind}</td>
      <td>
        <Badge tone={statusTone(row.display_state)}>{statusWord(row.display_state, row.kind)}</Badge>
      </td>
      <td>{(row.owners ?? []).map((key) => entityName(view, key)).join(", ") || <span className="muted small">none</span>}</td>
      <td className="mono">{row.due ?? <span className="muted small">none</span>}</td>
      <td className="mono">{row.slack_days ?? <span className="muted small">none</span>}</td>
      <td className="mono">{row.gravity.toFixed(1)}</td>
      <td className="mono">{row.leverage.toFixed(1)}</td>
      <td className="mono">{row.rank == null ? "" : row.rank.rank.toFixed(3)}</td>
      <td>
        <Flags view={view} row={row} />
      </td>
    </tr>
  );
}

function Table({ view, rows, settings, chosen, onToggle }: { view: Ready; rows: NodeRow[]; settings: ListSettings; chosen: Set<string>; onToggle: (key: string) => void }) {
  const groups = settings.grouped ? byContainer(rows) : [{ container: undefined, path: [], rows }];
  return (
    <table className="data list-table" data-testid="list-table">
      <thead>
        <tr>
          <th aria-label="Selected" />
          {COLUMNS.map((column) => (
            <th key={column}>{column}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {groups.map((group) => (
          <Fragment key={group.container ?? ""}>
            {settings.grouped ? (
              <tr className="list-group" data-testid="list-group" data-node={group.container ?? ""}>
                <th colSpan={COLUMNS.length + 1}>{group.container === undefined ? "Top of the journey" : [...group.path].map((key) => view.journey.graph.nodes?.find((node) => node.key === key)?.title ?? key).join(" / ")}</th>
              </tr>
            ) : null}
            {group.rows.map((row) => (
              <Row key={row.key} view={view} row={row} grouped={settings.grouped} chosen={chosen.has(row.key)} onToggle={() => { onToggle(row.key); }} />
            ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}

/** PLAN, LIST: keyed by the address's query by its caller, so a new query starts from its first page with nothing selected. */
export function ListBody({ view, settings, onSettings }: { view: Ready; settings: ListSettings; onSettings: (next: ListSettings) => void }) {
  const [cursors, setCursors] = useState<number[]>([]);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const cursor = cursors.at(-1);
  const request = useMemo(() => ({ projection: "list" as const, query: listQueryOf(settings, cursor) }), [settings, cursor]);
  const { value: page, error } = useProjected(view, request);
  const rows = page?.rows ?? [];
  // The selection outlives paging, so a bulk action covers every node selected on any page.
  const selected = selectionOf(view, chosen);
  const toggle = (key: string) => {
    const next = new Set(chosen);
    if (!next.delete(key)) {
      next.add(key);
    }
    setChosen(next);
  };
  return (
    <section className="stack" aria-label="List" data-testid="list">
      {error === undefined ? null : <p className="callout callout-bad">The list could not be read: {error}</p>}
      <div className="row">
        <span className="muted small" data-testid="list-total" data-total={page?.total ?? ""}>
          {page === undefined ? "Reading the list..." : `${String(page.total)} nodes match; showing ${String(rows.length)}.`}
        </span>
        <SortSelect sort={settings.sort} onChange={(sort) => { onSettings({ ...settings, sort }); }} />
        <Check label="Group by container" checked={settings.grouped} testId="grouped" onChange={(grouped) => { onSettings({ ...settings, grouped }); }} />
        <Button onClick={() => { setChosen(new Set(rows.map((row) => row.key))); }}>Select all shown</Button>
        <Button onClick={() => { setChosen(new Set()); }}>Clear the selection</Button>
        {cursors.length === 0 ? null : <Button onClick={() => { setCursors(cursors.slice(0, -1)); }}>Previous page</Button>}
        {page?.next == null ? null : <Button onClick={() => { setCursors([...cursors, page.next ?? 0]); }}>Next page</Button>}
      </div>
      {selected.length === 0 ? null : <BulkBar view={view} selected={selected} hidden={selected.filter((facts) => !rows.some((row) => row.key === facts.node.key)).length} onLanded={() => { setChosen(new Set()); }} />}
      <Table view={view} rows={rows} settings={settings} chosen={chosen} onToggle={toggle} />
    </section>
  );
}
