// C9: the Plan list. By default the journey as a tree in plan order, folded to its top level,
// each container saying how far along it is; opening every level is the flat table. Sorting a
// column (its header) flattens it and puts each row's container under its title. Not-relevant
// rows are left out unless asked for, so the list holds what the plan's count does. With
// DECISIONS on the rows are decisions with their answer, why, and what the answer affects
// (C12). Hovering a row brings up its checkbox, and a selection turns the header into the
// bulk actions (BulkBar.tsx). It is PLAN, LIST: the journey page (screens/JourneyFrame.tsx)
// holds its toolbar and the filter it opens (ListFilters).
import { useMemo, useState } from "react";
import { useNavigate } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import { decisionRows } from "../decisions/model.ts";
import { type Ready } from "../detail/model.ts";
import { ancestorsOf } from "../plan/tree.ts";
import { Menu } from "../screens/Menu.tsx";
import { Button } from "../ui/kit.tsx";
import { selectionOf } from "./acts.ts";
import { listPath, listQueryOf, type ListSettings } from "./address.ts";
import { BulkBar } from "./BulkBar.tsx";
import { Checks } from "./Controls.tsx";
import { Cell, type CellContext } from "./ListCells.tsx";
import { addableColumns, columnsOf, columnWords, flatLines, givesWay, rankPositions, SORT_OF, treeLines, type Column, type Line } from "./listModel.ts";
import { useRows } from "./useRows.ts";
import "./acting.css";

/** Whether any filter narrows the list: a narrowed tree opens, so every match is in sight. */
function narrowed(settings: ListSettings): boolean {
  return settings.flags.length + settings.states.length + settings.kinds.length > 0 || settings.within !== undefined || settings.owner !== undefined || settings.text.trim() !== "";
}

/** The columns held at the left while the table scrolls sideways. */
function pinned(column: Column): string | undefined {
  return column === "status" || column === "title" ? "list-pinned" : undefined;
}

function Head({ column, settings, onSort }: { column: Column; settings: ListSettings; onSort: (sort: ListSettings["sort"]) => void }) {
  const sort = SORT_OF[column];
  const words = columnWords(column, settings.decisions);
  if (sort === undefined) {
    return <th scope="col" data-column={column} data-gives-way={givesWay(column)} className={pinned(column)}>{words}</th>;
  }
  const active = settings.sort === sort;
  return (
    <th scope="col" aria-sort={active ? "descending" : "none"} data-column={column} data-gives-way={givesWay(column)} data-testid={`sort-${column}`}>
      <button type="button" className="list-sort" aria-label={`Sort by ${words.toLowerCase()}`} onClick={() => { onSort(active ? undefined : sort); }}>
        {words}
        {active ? <span aria-hidden="true"> ▾</span> : null}
      </button>
    </th>
  );
}

function Row({ line, columns, context, chosen, current, onToggle }: { line: Line; columns: Column[]; context: CellContext; chosen: boolean; current: boolean; onToggle: () => void }) {
  const { row } = line;
  return (
    <tr
      data-testid={row === undefined ? "list-context" : "list-row"}
      data-node={line.key}
      data-depth={line.depth}
      data-slack={row?.slack_days ?? ""}
      data-selected={chosen}
      aria-selected={current}
    >
      <td className="list-check list-pinned">
        {row === undefined ? null : <input type="checkbox" className="list-checkbox" aria-label={`Select ${row.title}`} checked={chosen} onChange={onToggle} />}
      </td>
      {columns.map((column) => (
        <td key={column} data-column={column} data-gives-way={givesWay(column)} className={[column === "title" ? "list-title-cell" : "list-cell", pinned(column)].filter(Boolean).join(" ")}>
          <Cell column={column} line={line} context={context} />
        </td>
      ))}
    </tr>
  );
}

/** The table: its header (the column labels, or the bulk actions once something is selected) and its lines. */
function Table({ view, settings, lines, columns, context, chosen, selected, everyLine, onToggle, onClear, onSelectAll }: {
  view: Ready;
  settings: ListSettings;
  lines: Line[];
  columns: Column[];
  context: CellContext;
  chosen: Set<string>;
  selected: string | undefined;
  everyLine: string[];
  onToggle: (key: string) => void;
  onClear: () => void;
  onSelectAll: () => void;
}) {
  const navigate = useNavigate();
  const picked = selectionOf(view, chosen);
  return (
    <table className="data list-table" data-testid="list-table" data-selecting={chosen.size > 0}>
      <thead>
        {picked.length > 0 ? (
          <tr>
            <th colSpan={columns.length + 1} className="bulk-head">
              <BulkBar view={view} selected={picked} hidden={picked.filter((facts) => !everyLine.includes(facts.node.key)).length} onClear={onClear} onLanded={onClear} />
            </th>
          </tr>
        ) : (
          <tr>
            <th scope="col" className="list-check list-pinned">
              <input type="checkbox" className="list-checkbox" aria-label="Select every row shown" checked={false} onChange={onSelectAll} />
            </th>
            {columns.map((column) => (
              <Head key={column} column={column} settings={settings} onSort={(sort) => void navigate(listPath(view.journey.header.id, { ...settings, sort }, selected))} />
            ))}
          </tr>
        )}
      </thead>
      <tbody>
        {lines.map((line) => (
          <Row key={line.key} line={line} columns={columns} context={context} chosen={chosen.has(line.key)} current={line.key === selected} onToggle={() => { onToggle(line.key); }} />
        ))}
      </tbody>
    </table>
  );
}

/** What the strip above the table offers: its columns, the tree's fold, and the not-relevant rows. */
function Strip({ view, settings, total, hidden, tree, all, onAll, selectedNode }: { view: Ready; settings: ListSettings; total: number | undefined; hidden: number; tree: boolean; all: boolean; onAll: () => void; selectedNode: string | undefined }) {
  const navigate = useNavigate();
  const journey = view.journey.header.id;
  const go = (next: ListSettings) => void navigate(listPath(journey, next, selectedNode));
  return (
    <div className="list-strip row">
      <Menu
        label="Columns"
        testId="columns-button"
        role="dialog"
        trigger="Columns ▾"
      >
        {() => (
          <Checks
            legend="Show"
            options={addableColumns(settings.decisions)}
            chosen={settings.columns}
            words={(column) => columnWords(column, settings.decisions)}
            testId="column"
            stacked
            onChange={(columns) => { go({ ...settings, columns }); }}
          />
        )}
      </Menu>
      {tree ? (
        <Button ghost data-testid="list-fold-all" onClick={onAll}>
          {all ? "Fold to the top level" : "Open every level"}
        </Button>
      ) : null}
      <span className="muted" data-testid="list-total" data-total={total ?? ""}>
        {total === undefined ? "Reading the list..." : `${String(total)} ${settings.decisions ? "decision" : "item"}${total === 1 ? "" : "s"}`}
      </span>
      {hidden === 0 && !settings.notRelevant ? null : (
        <Button ghost data-testid="list-not-relevant" aria-pressed={settings.notRelevant} onClick={() => { go({ ...settings, notRelevant: !settings.notRelevant }); }}>
          {settings.notRelevant ? `Hide the ${String(hidden)} not relevant` : `Show ${String(hidden)} not relevant`}
        </Button>
      )}
    </div>
  );
}

/** A set of keys with the one change a click makes: in if out, out if in. */
function useKeys(): [Set<string>, (key: string) => void, (keys: Iterable<string>) => void] {
  const [keys, setKeys] = useState<Set<string>>(new Set());
  const toggle = (key: string) => {
    const next = new Set(keys);
    if (!next.delete(key)) {
      next.add(key);
    }
    setKeys(next);
  };
  return [keys, toggle, (replacement) => { setKeys(new Set(replacement)); }];
}

/** PLAN, LIST: keyed by the address's query by its caller, so a new query starts with nothing selected and the tree folded. */
export function ListBody({ view, settings, selected }: { view: Ready; settings: ListSettings; selected?: string | undefined }) {
  const [chosen, toggle, choose] = useKeys();
  const [flipped, fold, setFlipped] = useKeys();
  const [everyLevel, setEveryLevel] = useState<boolean | undefined>(undefined);
  const query = useMemo(() => listQueryOf(settings), [settings]);
  const { rows, error } = useRows(view, query);
  // What Show not relevant would add under the same filters, so the button never promises rows the filters would drop.
  const ruledOut = useRows(view, useMemo(() => ({ ...query, display_states: ["not_relevant" as const] }), [query])).rows?.length ?? 0;
  const needsDecisions = settings.decisions || settings.columns.some((column) => ["answer", "why", "affects"].includes(column));
  const projected = useProjected(view, needsDecisions ? { projection: "decision_view" as const } : undefined);
  const decisions = useMemo(() => new Map((projected.value === undefined ? [] : decisionRows(view, projected.value)).map((decision) => [decision.key, decision])), [view, projected.value]);
  const ranks = useMemo(() => rankPositions(view), [view]);
  const tree = settings.sort === undefined && !settings.decisions;
  // The open node's containers stay open, so it is not hidden in a fold; a click flips what the rest would do.
  const holding = useMemo(() => new Set(selected === undefined ? [] : ancestorsOf(view, selected)), [view, selected]);
  const open = everyLevel ?? narrowed(settings);
  const lines = useMemo(
    () => (rows === undefined ? [] : tree ? treeLines(view, rows, (key) => (open || holding.has(key)) !== flipped.has(key)) : flatLines(view, rows, settings.sort)),
    [view, rows, tree, settings.sort, open, holding, flipped],
  );
  const everyLine = lines.flatMap((line) => (line.row === undefined ? [] : [line.row.key]));
  const columns = columnsOf(settings);
  const context: CellContext = { view, tree, decisions, ranks, onFold: fold };
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      {error === undefined ? null : <p className="callout callout-bad">The list could not be read: {error}</p>}
      <Strip
        view={view}
        settings={settings}
        total={rows?.length}
        hidden={ruledOut}
        tree={tree}
        all={open}
        selectedNode={selected}
        onAll={() => {
          setEveryLevel(!open);
          setFlipped([]);
        }}
      />
      <Table
        view={view}
        settings={settings}
        lines={lines}
        columns={columns}
        context={context}
        chosen={chosen}
        selected={selected}
        everyLine={everyLine}
        onToggle={toggle}
        onClear={() => { choose([]); }}
        onSelectAll={() => { choose(everyLine); }}
      />
      {rows !== undefined && lines.length === 0 ? (
        <p className="muted" data-testid="list-empty">Nothing matches. Change the filter to see more.</p>
      ) : null}
    </section>
  );
}
