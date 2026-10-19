// C9: the list. The journey as a tree of the nodes the level draws (journeys/requests.ts: Depth's
// step, the Filter and the search, the same level every view draws from), folded as the
// ladder says and the viewer flips, each container saying how far along it is. Sorting a column
// (its header) flattens it and puts each row's container under its title; every drawn node keeps
// its row. With the Frontier chip on, the rows the frontier does not light fade, the frontier's
// own rows say why they are there with their primary action under the title (C10), and under
// the table is what needs a look. With Depth's Fade on every row is drawn and those below the
// depth fade. At Decisions depth the rows show each decision's answer, why, and what the answer
// affects (C12). Hovering a row brings up its checkbox; the rows checked
// are the graph's one selection (canvas/selection.ts), acted on in the inspector. The journey
// page (screens/JourneyFrame.tsx) holds the view state the toolbar changes.
import { useEffect, useMemo, useRef, type CSSProperties, type MouseEvent } from "react";
import { useLocation, useNavigate } from "react-router";

import { useFolds, useLevel, useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import { collapsedAt, drawnStep, stepOf } from "../canvas/ladder.ts";
import type { Selection } from "../canvas/selection.ts";
import { decisionRows } from "../decisions/model.ts";
import type { Ready } from "../detail/model.ts";
import { useMineOf } from "../journeys/mine.ts";
import { badgesOf, currentStagesOf, depthFaded, levelRequestOf, lightsUp, litOf, outOfFocus, stepHides, tableQueryOf } from "../journeys/requests.ts";
import type { ViewState } from "../journeys/view.ts";
import { containerKeys, planOrder } from "../plan/tree.ts";
import { Menu } from "../screens/Menu.tsx";
import { journeySubject, NothingMatches, ViewBar } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { withListSettings, type ListSettings } from "./address.ts";
import { Checks } from "./Controls.tsx";
import { Cell, type CellContext } from "./ListCells.tsx";
import { Folds } from "./Folds.tsx";
import { NextForYou } from "./NextForYou.tsx";
import { StalledPanel } from "./Stalled.tsx";
import { addableColumns, columnsOf, columnWords, flatLines, givesWay, rankPositions, SORT_OF, treeLines, type Column, type Line } from "./listModel.ts";
import { useRows } from "./useRows.ts";
import "./acting.css";
import { ReadFailed } from "../ui/ReadFailed.tsx";

/** The columns held at the left while the table scrolls sideways. */
function pinned(column: Column, columns: readonly Column[]): string | undefined {
  return column === "status" || column === "title" || (column === "rank" && columns[0] === "rank") ? "list-pinned" : undefined;
}

/** A click on the row itself, not on a control inside it, selects it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

function Head({ column, columns, list, onSort }: { column: Column; columns: readonly Column[]; list: ListSettings; onSort: (sort: ListSettings["sort"]) => void }) {
  const sort = SORT_OF[column];
  const words = columnWords(column);
  if (sort === undefined) {
    return <th scope="col" data-column={column} data-gives-way={givesWay(column)} className={pinned(column, columns)}>{words}</th>;
  }
  const active = list.sort === sort;
  return (
    <th scope="col" aria-sort={active ? "descending" : "none"} data-column={column} data-gives-way={givesWay(column)} className={pinned(column, columns)} data-testid={`sort-${column}`}>
      <button type="button" className="list-sort" aria-label={`Sort by ${words.toLowerCase()}`} onClick={() => { onSort(active ? undefined : sort); }}>
        {words}
        {active ? <span aria-hidden="true"> ▾</span> : null}
      </button>
    </th>
  );
}

function Row({ line, columns, context, selection, current, faded }: { line: Line; columns: Column[]; context: CellContext; selection: Selection; current: boolean; faded: boolean }) {
  const { row } = line;
  const chosen = selection.members.length > 1 && selection.members.includes(line.key);
  const navigate = useNavigate();
  const frame = useFrameActions();
  const addresses = useNodeAddresses();
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
      data-faded={faded ? true : undefined}
      data-frontier={context.frontier?.has(line.key) === true ? true : undefined}
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
          void navigate(addresses.open(line.key));
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
function Table({ list, lines, columns, context, selection, selected, everyLine, outOfFocus }: {
  list: ListSettings;
  lines: Line[];
  columns: Column[];
  context: CellContext;
  selection: Selection;
  selected: string | undefined;
  everyLine: string[];
  /** Whether a row is dimmed: out of focus for Frontier or for Depth's Fade. */
  outOfFocus: (key: string) => boolean;
}) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  const all = everyLine.length > 0 && everyLine.every((key) => selection.members.includes(key));
  return (
    <table className="data list-table" data-selecting={selection.members.length > 1} data-ranked={columns[0] === "rank"} data-testid="list-table">
      <thead>
        <tr>
          <th scope="col" className="list-check list-pinned">
            <input type="checkbox" className="list-checkbox" disabled={everyLine.length === 0} aria-label="Select every row shown" checked={all} onChange={() => { selection.set(all ? [] : everyLine); }} />
          </th>
          {columns.map((column) => (
            <Head key={column} column={column} columns={columns} list={list} onSort={(sort) => void navigate(`${pathname}${withListSettings(search, { ...list, sort })}`)} />
          ))}
        </tr>
      </thead>
      <tbody>
        {lines.map((line) => (
          <Row key={line.key} line={line} columns={columns} context={context} selection={selection} current={line.key === selected} faded={outOfFocus(line.key)} />
        ))}
      </tbody>
    </table>
  );
}

/** The list's count and Columns, which ride the end of the view controls' row. */
function RowEnd({ list, decisionColumns, total }: { list: ListSettings; decisionColumns: boolean; total: number | undefined }) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  return (
    <>
      <span className="muted" data-testid="list-total" data-total={total ?? ""}>
        {total === undefined ? "Reading the list..." : `${String(total)} item${total === 1 ? "" : "s"}`}
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
            options={addableColumns(decisionColumns)}
            chosen={list.columns}
            words={columnWords}
            testId="column"
            stacked
            onChange={(columns) => { void navigate(`${pathname}${withListSettings(search, { ...list, columns })}`); }}
          />
        )}
      </Menu>
    </>
  );
}

/** C10: under the table while Frontier is on, what the viewer holds that cannot be done yet, what needs a look, what is snoozed, and, when nothing can move, why. */
function FrontierPanels({ view, drawn }: { view: Ready; drawn: ReadonlySet<string> | undefined }) {
  const addresses = useNodeAddresses();
  const mine = useMineOf(view.journey.header.id);
  // The stalled panel already says what the journey waits on, so the line that says the same is left out.
  const stalled = view.derived.stalled != null;
  const entries = mine.status === "ready" ? mine.entries.filter((entry) => drawn?.has(entry.node) === true) : [];
  return (
    <>
      {!stalled ? <NextForYou view={view} entries={entries} /> : null}
      {stalled ? <StalledPanel view={view} /> : null}
      <Folds view={view} drawn={drawn} toOf={(key) => addresses.open(key)} />
    </>
  );
}

/**
 * PLAN, LIST: the rows the level draws, in the tree or sorted; the tree keeps the viewer's folds
 * across a filter and an emphasis; the rows checked are the journey's one selection.
 */
export function ListBody({ view, state, picks, list, selected, selection }: { view: Ready; state: ViewState; picks: number; list: ListSettings; selected?: string | undefined; selection: Selection }) {
  const nodes = view.journey.graph.nodes ?? [];
  const depth = stepOf({ step: state.depth }, nodes);
  const shown = drawnStep(depth, state.fade);
  const request = useMemo(() => levelRequestOf(state, shown, []), [state, shown]);
  const { level, drawn, error: levelError } = useLevel(view, request, selected);
  const query = useMemo(() => tableQueryOf(list.sort), [list.sort]);
  const { rows, error: rowsError } = useRows(view, query);
  const decisionColumns = depth === "decisions";
  const needsDecisions = decisionColumns || list.columns.some((column) => ["answer", "why", "affects"].includes(column));
  const projected = valueOf(useProjected(view, needsDecisions ? { projection: "decision_view" as const } : undefined));
  const decisions = useMemo(() => new Map((projected === undefined ? [] : decisionRows(view, projected)).map((decision) => [decision.key, decision])), [view, projected]);
  const ranks = useMemo(() => rankPositions(view, state.frontier), [view, state.frontier]);
  const order = useMemo(() => new Map(planOrder(view).map((key, at) => [key, at])), [view]);
  const containers = useMemo(() => containerKeys(nodes), [nodes]);
  const tree = list.sort === undefined;
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const current = useMemo(() => currentStagesOf(view), [view]);
  const folds = useFolds(nodes, current, state.depth, shown, picks, selected, (key) => void navigate(addresses.open(key)));
  const kept = useMemo(() => (rows === undefined || drawn === undefined ? undefined : rows.filter((row) => drawn.has(row.key))), [rows, drawn]);
  const lines = useMemo(
    () => (kept === undefined || level === undefined ? [] : tree ? treeLines(level.nodes, new Map(kept.map((row) => [row.key, row])), folds.open, order, containers) : flatLines(view, kept, list.sort)),
    [view, kept, level, tree, list.sort, folds.open, order, containers],
  );
  const base = useMemo(() => litOf("list", state, view), [state, view]);
  const lit = useMemo(() => (base === undefined ? undefined : lightsUp(nodes, base)), [nodes, base]);
  const dimmed = useMemo(() => {
    const hidden = collapsedAt(depth, nodes, current, [], []);
    return outOfFocus(lit, drawn === undefined ? new Set() : depthFaded(state, nodes, depth, hidden, drawn));
  }, [state, current, nodes, depth, drawn, lit]);
  const frontier = useMemo(() => (state.frontier ? new Set(view.derived.acting_frontier) : undefined), [state.frontier, view.derived.acting_frontier]);
  const badges = useMemo(() => badgesOf("list", level?.hidden_by_class ?? {}, base, stepHides(nodes, depth, []), state.fade), [level, base, nodes, depth, state.fade]);
  const everyLine = lines.flatMap((line) => (line.row === undefined ? [] : [line.row.key]));
  const columns = columnsOf(list, decisionColumns);
  const context: CellContext = { view, tree, decisions, ranks, frontier, sort: list.sort, onFold: folds.fold };
  const error = levelError ?? rowsError;
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar subject={journeySubject(view)} screen="list" badges={badges} end={<RowEnd list={list} decisionColumns={decisionColumns} total={kept?.length} />} />
      {error === undefined ? null : <p className="callout callout-bad"><ReadFailed what="The list" error={error} /></p>}
      <div className="node-panel list-panel" data-empty={lines.length === 0 ? true : undefined} data-testid="list-panel">
        <Table list={list} lines={lines} columns={columns} context={context} selection={selection} selected={selected} everyLine={everyLine} outOfFocus={dimmed} />
        {kept !== undefined && level !== undefined && lines.length === 0 ? (
          <div className="empty-row" data-testid="list-empty">
            <NothingMatches />
          </div>
        ) : null}
      </div>
      {state.frontier ? <FrontierPanels view={view} drawn={drawn} /> : null}
    </section>
  );
}
