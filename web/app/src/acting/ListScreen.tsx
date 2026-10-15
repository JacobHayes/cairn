// C9: the list. By default the journey as a tree in plan order, folded to its top level,
// each container saying how far along it is; opening every level is the flat table. Sorting a
// column (its header) flattens it and puts each row's container under its title. With the
// Frontier chip on it is the frontier: only what can move now, flat in rank order, each row
// with its owner and primary action, and under the table what needs a look (C10). Not-relevant
// rows are left out unless asked for, so the list holds what the plan's count does. With
// DECISIONS on the rows are decisions with their answer, why, and what the answer affects
// (C12). Hovering a row brings up its checkbox; the rows checked are the graph's one selection
// (canvas/selection.ts), acted on in the inspector. The journey page (screens/JourneyFrame.tsx)
// holds its toolbar and the filter it opens (ListFilters).
import { useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import { useLocation, useNavigate } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import type { Selection } from "../canvas/selection.ts";
import { decisionRows } from "../decisions/model.ts";
import { type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { useMineOf } from "../journeys/mine.ts";
import { ancestorsOf } from "../plan/tree.ts";
import { Menu } from "../screens/Menu.tsx";
import { ViewBar } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { listPath, listQueryOf, type ListSettings } from "./address.ts";
import { Checks } from "./Controls.tsx";
import { Cell, type CellContext } from "./ListCells.tsx";
import { Folds } from "./Folds.tsx";
import { NextForYou } from "./NextForYou.tsx";
import { StalledPanel } from "./Stalled.tsx";
import { addableColumns, columnsOf, columnWords, flatLines, givesWay, rankPositions, SORT_OF, treeLines, type Column, type Line } from "./listModel.ts";
import { useRows } from "./useRows.ts";
import "./acting.css";

/** Whether any filter narrows the list: a narrowed tree opens, so every match is in sight. */
function narrowed(settings: ListSettings): boolean {
  return settings.flags.length + settings.states.length + settings.kinds.length > 0 || settings.within !== undefined || settings.owner !== undefined || settings.text.trim() !== "";
}

/** The columns held at the left while the table scrolls sideways. */
function pinned(column: Column, columns: readonly Column[]): string | undefined {
  return column === "status" || column === "title" || (column === "rank" && columns[0] === "rank") ? "list-pinned" : undefined;
}

/** A click on the row itself, not on a control inside it, selects it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

function Head({ column, columns, settings, onSort }: { column: Column; columns: readonly Column[]; settings: ListSettings; onSort: (sort: ListSettings["sort"]) => void }) {
  const sort = SORT_OF[column];
  const words = columnWords(column, settings.decisions);
  if (sort === undefined) {
    return <th scope="col" data-column={column} data-gives-way={givesWay(column)} className={pinned(column, columns)}>{words}</th>;
  }
  const active = settings.sort === sort;
  return (
    <th scope="col" aria-sort={active ? "descending" : "none"} data-column={column} data-gives-way={givesWay(column)} className={pinned(column, columns)} data-testid={`sort-${column}`}>
      <button type="button" className="list-sort" aria-label={`Sort by ${words.toLowerCase()}`} onClick={() => { onSort(active ? undefined : sort); }}>
        {words}
        {active ? <span aria-hidden="true"> ▾</span> : null}
      </button>
    </th>
  );
}

function Row({ line, columns, context, selection, current }: { line: Line; columns: Column[]; context: CellContext; selection: Selection; current: boolean }) {
  const { row } = line;
  const chosen = selection.members.length > 1 && selection.members.includes(line.key);
  const navigate = useNavigate();
  const frame = useFrameActions();
  const { pathname, search } = useLocation();
  const item = useRef<HTMLTableRowElement>(null);
  // j and k move the selection: keep the selected row in view.
  useEffect(() => {
    if (current) {
      item.current?.scrollIntoView({ block: "nearest" });
    }
  }, [current]);
  return (
    <tr
      ref={item}
      data-testid={row === undefined ? "list-context" : "list-row"}
      data-node={line.key}
      data-depth={line.depth}
      data-container={line.container}
      data-state={row?.display_state ?? ""}
      style={{ "--depth": line.depth } as CSSProperties}
      data-slack={row?.slack_days ?? ""}
      data-selected={chosen}
      aria-selected={current}
      onClick={(event) => {
        // The row's own title link is the row clicked; any other link or control is its own.
        const own = event.target instanceof Element && event.target.closest("a")?.getAttribute("data-node") === line.key;
        if (row === undefined || !(own || onRow(event))) {
          return;
        }
        if (event.shiftKey || event.metaKey || event.ctrlKey) {
          event.preventDefault();
          selection.toggle(line.key);
          return;
        }
        selection.release();
        if (!own) {
          // The selected row clicked again changes no address, so the inspector must be asked to come back.
          frame?.show("inspector");
          void navigate(`${nodePath(screenPath(pathname), line.key)}${search}`);
        }
      }}
    >
      <td className="list-check list-pinned">
        {row === undefined ? null : <input type="checkbox" className="list-checkbox" aria-label={`Select ${row.title}`} checked={selection.members.includes(line.key)} onChange={() => { selection.toggle(line.key); }} />}
      </td>
      {columns.map((column) => (
        <td key={column} data-column={column} data-gives-way={givesWay(column)} className={[column === "title" ? "list-title-cell" : "list-cell", pinned(column, columns)].filter(Boolean).join(" ")}>
          <div className="list-content">
            <Cell column={column} line={line} context={context} />
          </div>
        </td>
      ))}
    </tr>
  );
}

/** The table: its header (the column labels) and its lines. */
function Table({ view, settings, lines, columns, context, selection, selected, everyLine }: {
  view: Ready;
  settings: ListSettings;
  lines: Line[];
  columns: Column[];
  context: CellContext;
  selection: Selection;
  selected: string | undefined;
  everyLine: string[];
}) {
  const navigate = useNavigate();
  const all = everyLine.length > 0 && everyLine.every((key) => selection.members.includes(key));
  return (
    <table className="data list-table" data-selecting={selection.members.length > 1} data-ranked={columns[0] === "rank"} data-testid="list-table">
      <thead>
        <tr>
          <th scope="col" className="list-check list-pinned">
            <input type="checkbox" className="list-checkbox" aria-label="Select every row shown" checked={all} onChange={() => { selection.set(all ? [] : everyLine); }} />
          </th>
          {columns.map((column) => (
            <Head key={column} column={column} columns={columns} settings={settings} onSort={(sort) => void navigate(listPath(view.journey.header.id, { ...settings, sort }, selected))} />
          ))}
        </tr>
      </thead>
      <tbody>
        {lines.map((line) => (
          <Row key={line.key} line={line} columns={columns} context={context} selection={selection} current={line.key === selected} />
        ))}
      </tbody>
    </table>
  );
}

/** The list's tree fold, count and Columns, which ride the end of the view controls' row. */
function RowEnd({ view, settings, total, selectedNode, fold }: { view: Ready; settings: ListSettings; total: number | undefined; selectedNode: string | undefined; fold: { all: boolean; onAll: () => void } | undefined }) {
  const navigate = useNavigate();
  const journey = view.journey.header.id;
  return (
    <>
      {fold === undefined ? null : (
        <Button ghost data-testid="list-fold-all" onClick={fold.onAll}>
          {fold.all ? "Fold to the top level" : "Open every level"}
        </Button>
      )}
      <span className="muted" data-testid="list-total" data-total={total ?? ""}>
        {total === undefined ? "Reading the list..." : `${String(total)} ${settings.decisions ? "decision" : "item"}${total === 1 ? "" : "s"}`}
      </span>
      <Menu
        label="Columns"
        testId="columns-button"
        role="dialog"
        align="end"
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
            onChange={(columns) => { void navigate(listPath(journey, { ...settings, columns }, selectedNode)); }}
          />
        )}
      </Menu>
    </>
  );
}

/** C10: under the frontier table, what the viewer holds that cannot be done yet, what needs a look, what is snoozed, and, when nothing can move, why. */
function FrontierPanels({ view, settings, empty, stalled: held }: { view: Ready; settings: ListSettings; empty: boolean; stalled: boolean }) {
  const { pathname, search } = useLocation();
  const mine = useMineOf(view.journey.header.id);
  // The stalled panel already says what the journey waits on, so the line that says the same is left out.
  const stalled = empty && held;
  const mineNarrowing = { mine: settings.flags.includes("mine"), kinds: settings.kinds, decisions: settings.decisions, text: settings.text };
  return (
    <>
      {mine.status === "ready" && !stalled ? <NextForYou view={view} entries={mine.entries} /> : null}
      {stalled ? <StalledPanel view={view} /> : null}
      <Folds view={view} settings={mineNarrowing} toOf={(key) => `${nodePath(screenPath(pathname), key)}${search}`} />
    </>
  );
}

/**
 * PLAN, LIST: the tree keeps its open and folded levels, which are the viewer's, across a
 * filter and a row-set toggle; the rows checked are the journey's one selection.
 */
export function ListBody({ view, settings, selected, selection }: { view: Ready; settings: ListSettings; selected?: string | undefined; selection: Selection }) {
  const [folds, setFolds] = useState<{ by: ReadonlyMap<string, boolean>; every: boolean | undefined }>({ by: new Map(), every: undefined });
  const query = useMemo(() => listQueryOf(settings), [settings]);
  const { rows, error } = useRows(view, query);
  // What Show not relevant would add under the same filters: the Filter badge says how many rows it leaves out.
  const ruledOut = useRows(view, useMemo(() => ({ ...query, display_states: ["not_relevant" as const] }), [query])).rows?.length ?? 0;
  const needsDecisions = settings.decisions || settings.columns.some((column) => ["answer", "why", "affects"].includes(column));
  const projected = useProjected(view, needsDecisions ? { projection: "decision_view" as const } : undefined);
  const decisions = useMemo(() => new Map((projected.value === undefined ? [] : decisionRows(view, projected.value)).map((decision) => [decision.key, decision])), [view, projected.value]);
  // Under the Frontier lens the places are the engine's order of the frontier, its ties broken as it breaks them (the graph's strip says them too).
  const frontier = useProjected(view, settings.frontier ? { projection: "next" as const } : undefined);
  const ranks = useMemo(() => (settings.frontier ? new Map((frontier.value?.items ?? []).map((row, at) => [row.key, at + 1])) : rankPositions(view)), [view, settings.frontier, frontier.value]);
  const tree = settings.sort === undefined && !settings.decisions && !settings.frontier;
  // The open node's containers stay open, so it is not hidden in a fold; a click flips what the rest would do.
  const holding = useMemo(() => new Set(selected === undefined ? [] : ancestorsOf(view, selected)), [view, selected]);
  const open = folds.every ?? narrowed(settings);
  const isOpen = (key: string) => folds.by.get(key) ?? (open || holding.has(key));
  const lines = useMemo(
    () => (rows === undefined ? [] : tree ? treeLines(view, rows, (key) => folds.by.get(key) ?? (open || holding.has(key))) : flatLines(view, rows, settings.sort ?? (settings.frontier ? "rank" : undefined))),
    [view, rows, tree, settings.sort, open, holding, folds.by],
  );
  const everyLine = lines.flatMap((line) => (line.row === undefined ? [] : [line.row.key]));
  const columns = columnsOf(settings);
  const context: CellContext = { view, tree, decisions, ranks, acting: settings.frontier, sort: settings.sort, onFold: (key) => { setFolds({ ...folds, by: new Map(folds.by).set(key, !isOpen(key)) }); } };
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar
        ready={view}
        screen="list"
        node={selected}
        badges={ruledOut === 0 || settings.notRelevant ? undefined : { filter: { count: ruledOut, words: `${String(ruledOut)} not relevant hidden` } }}
        end={
          <RowEnd
            view={view}
            settings={settings}
            total={rows?.length}
            selectedNode={selected}
            fold={tree ? { all: open, onAll: () => { setFolds({ by: new Map(), every: !open }); } } : undefined}
          />
        } />
      {error === undefined ? null : <p className="callout callout-bad">The list could not be read: {error}</p>}
      <Table
        view={view}
        settings={settings}
        lines={lines}
        columns={columns}
        context={context}
        selection={selection}
        selected={selected}
        everyLine={everyLine}
      />
      {rows !== undefined && lines.length === 0 ? (
        <p className="muted" data-testid="list-empty">
          {!settings.frontier ? "Nothing matches. Change the filter to see more." : narrowed(settings) || settings.decisions ? "Nothing on the frontier matches these filters." : "Nothing is left to act on in this journey."}
        </p>
      ) : null}
      {settings.frontier ? <FrontierPanels view={view} settings={settings} empty={rows !== undefined && lines.length === 0} stalled={frontier.value?.stalled != null && frontier.value.items.length === 0} /> : null}
    </section>
  );
}
