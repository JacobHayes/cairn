// C9: the list. The journey as a tree of the nodes the level draws (journeys/requests.ts: the
// detail step, the Filter and the search, the same level every view draws from), folded as the
// ladder says and the viewer flips, each container saying how far along it is. Sorting a column
// (its header) flattens it and puts each row's container under its title; every drawn node keeps
// its row. With the Frontier chip on, the rows the frontier does not light fade, the frontier's
// own rows say why they are there with their primary action under the title (C10), and under
// the table is what needs a look. With DECISIONS on the rows are decisions with their answer,
// why, and what the answer affects (C12). Hovering a row brings up its checkbox; the rows checked
// are the graph's one selection (canvas/selection.ts), acted on in the inspector. The journey
// page (screens/JourneyFrame.tsx) holds the view state the toolbar changes.
import { useEffect, useMemo, useRef, type CSSProperties, type MouseEvent } from "react";
import { useLocation, useNavigate } from "react-router";

import { useFlips, useLevel, useProjected } from "../canvas/hooks.ts";
import { currentStages, openAt, stepOf } from "../canvas/ladder.ts";
import type { Selection } from "../canvas/selection.ts";
import { decisionRows } from "../decisions/model.ts";
import { recordOf, type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { useMineOf } from "../journeys/mine.ts";
import { badgesOf, levelRequestOf, lightsUp, litOf, stepHides, tableQueryOf } from "../journeys/requests.ts";
import type { ViewState } from "../journeys/view.ts";
import { ancestorsOf, planOrder } from "../plan/tree.ts";
import { Menu } from "../screens/Menu.tsx";
import { ResetView, ViewBar } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { withListSettings, type ListSettings } from "./address.ts";
import { Checks } from "./Controls.tsx";
import { Cell, type CellContext } from "./ListCells.tsx";
import { Folds } from "./Folds.tsx";
import { NextForYou } from "./NextForYou.tsx";
import { StalledPanel } from "./Stalled.tsx";
import { addableColumns, columnsOf, columnWords, flatLines, givesWay, rankPositions, SORT_OF, treeLines, type Column, type Line } from "./listModel.ts";
import { useRows } from "./useRows.ts";
import "./acting.css";

/** The columns held at the left while the table scrolls sideways. */
function pinned(column: Column, columns: readonly Column[]): string | undefined {
  return column === "status" || column === "title" || (column === "rank" && columns[0] === "rank") ? "list-pinned" : undefined;
}

/** A click on the row itself, not on a control inside it, selects it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

function Head({ column, columns, list, decisions, onSort }: { column: Column; columns: readonly Column[]; list: ListSettings; decisions: boolean; onSort: (sort: ListSettings["sort"]) => void }) {
  const sort = SORT_OF[column];
  const words = columnWords(column, decisions);
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

function Row({ line, columns, context, selection, current, lit }: { line: Line; columns: Column[]; context: CellContext; selection: Selection; current: boolean; lit: ReadonlySet<string> | undefined }) {
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
      data-lit={lit === undefined ? undefined : lit.has(line.key)}
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
function Table({ list, decisions, lines, columns, context, selection, selected, everyLine, lit }: {
  list: ListSettings;
  decisions: boolean;
  lines: Line[];
  columns: Column[];
  context: CellContext;
  selection: Selection;
  selected: string | undefined;
  everyLine: string[];
  lit: ReadonlySet<string> | undefined;
}) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  const all = everyLine.length > 0 && everyLine.every((key) => selection.members.includes(key));
  return (
    <table className="data list-table" data-selecting={selection.members.length > 1} data-ranked={columns[0] === "rank"} data-testid="list-table">
      <thead>
        <tr>
          <th scope="col" className="list-check list-pinned">
            <input type="checkbox" className="list-checkbox" aria-label="Select every row shown" checked={all} onChange={() => { selection.set(all ? [] : everyLine); }} />
          </th>
          {columns.map((column) => (
            <Head key={column} column={column} columns={columns} list={list} decisions={decisions} onSort={(sort) => void navigate(`${pathname}${withListSettings(search, { ...list, sort })}`)} />
          ))}
        </tr>
      </thead>
      <tbody>
        {lines.map((line) => (
          <Row key={line.key} line={line} columns={columns} context={context} selection={selection} current={line.key === selected} lit={lit} />
        ))}
      </tbody>
    </table>
  );
}

/** The list's count and Columns, which ride the end of the view controls' row. */
function RowEnd({ list, decisions, total }: { list: ListSettings; decisions: boolean; total: number | undefined }) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  return (
    <>
      <span className="muted" data-testid="list-total" data-total={total ?? ""}>
        {total === undefined ? "Reading the list..." : `${String(total)} ${decisions ? "decision" : "item"}${total === 1 ? "" : "s"}`}
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
            options={addableColumns(decisions)}
            chosen={list.columns}
            words={(column) => columnWords(column, decisions)}
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
function FrontierPanels({ view, decisions, drawn }: { view: Ready; decisions: boolean; drawn: ReadonlySet<string> | undefined }) {
  const { pathname, search } = useLocation();
  const mine = useMineOf(view.journey.header.id);
  // The stalled panel already says what the journey waits on, so the line that says the same is left out.
  const stalled = view.derived.stalled != null;
  const entries = mine.status === "ready" ? mine.entries.filter((entry) => drawn?.has(entry.node) === true) : [];
  return (
    <>
      {!stalled ? <NextForYou view={view} entries={entries} /> : null}
      {stalled ? <StalledPanel view={view} /> : null}
      <Folds view={view} decisions={decisions} drawn={drawn} toOf={(key) => `${nodePath(screenPath(pathname), key)}${search}`} />
    </>
  );
}

/** The containers the step opens and the viewer's flips since the step was chosen (5.1): a flip holds until another step. */
function useFolds(view: Ready, step: ViewState["step"], picks: number, selected: string | undefined) {
  const { flipped, flip } = useFlips(step, picks);
  const nodes = view.journey.graph.nodes ?? [];
  const shown = stepOf({ step }, nodes);
  const opens = useMemo(() => {
    const active = nodes.filter((node) => recordOf(view, node).state === "active").map((node) => node.key);
    return openAt(shown, nodes, currentStages(nodes, view.derived.acting_frontier, active));
  }, [view, nodes, shown]);
  // The open node's containers stay open, so it is not hidden in a fold.
  const holding = useMemo(() => new Set(selected === undefined ? [] : ancestorsOf(view, selected)), [view, selected]);
  const open = useMemo(() => (key: string) => holding.has(key) || opens(key) !== flipped.has(key), [holding, opens, flipped]);
  return { open, flip };
}

/**
 * PLAN, LIST: the rows the level draws, in the tree or sorted; the tree keeps the viewer's folds
 * across a filter and a highlight; the rows checked are the journey's one selection.
 */
export function ListBody({ view, state, picks, list, selected, selection }: { view: Ready; state: ViewState; picks: number; list: ListSettings; selected?: string | undefined; selection: Selection }) {
  const nodes = view.journey.graph.nodes ?? [];
  const step = stepOf({ step: state.step }, nodes);
  const request = useMemo(() => levelRequestOf(state, step, []), [state, step]);
  const { level, drawn, error: levelError } = useLevel(view, request);
  const query = useMemo(() => tableQueryOf(state, list.sort), [state, list.sort]);
  const { rows, error: rowsError } = useRows(view, query);
  const needsDecisions = state.decisions || list.columns.some((column) => ["answer", "why", "affects"].includes(column));
  const projected = useProjected(view, needsDecisions ? { projection: "decision_view" as const } : undefined);
  const decisions = useMemo(() => new Map((projected.value === undefined ? [] : decisionRows(view, projected.value)).map((decision) => [decision.key, decision])), [view, projected.value]);
  const ranks = useMemo(() => rankPositions(view, state.frontier), [view, state.frontier]);
  const order = useMemo(() => new Map(planOrder(view).map((key, at) => [key, at])), [view]);
  const tree = list.sort === undefined && !state.decisions;
  const folds = useFolds(view, state.step, picks, selected);
  const kept = useMemo(() => (rows === undefined || drawn === undefined ? undefined : rows.filter((row) => drawn.has(row.key))), [rows, drawn]);
  const lines = useMemo(
    () => (kept === undefined || level === undefined ? [] : tree ? treeLines(level.nodes, new Map(kept.map((row) => [row.key, row])), folds.open, order) : flatLines(view, kept, list.sort)),
    [view, kept, level, tree, list.sort, folds.open, order],
  );
  const base = useMemo(() => litOf("list", state, view), [state, view]);
  const lit = useMemo(() => (base === undefined ? undefined : lightsUp(nodes, base)), [nodes, base]);
  const frontier = useMemo(() => (state.frontier ? new Set(view.derived.acting_frontier) : undefined), [state.frontier, view.derived.acting_frontier]);
  const badges = useMemo(() => badgesOf("list", level?.hidden_by_class ?? {}, base, stepHides(nodes, step, [])), [level, base, nodes, step]);
  const everyLine = lines.flatMap((line) => (line.row === undefined ? [] : [line.row.key]));
  const columns = columnsOf(list, state.decisions);
  const context: CellContext = { view, tree, decisions, ranks, frontier, sort: list.sort, onFold: folds.flip };
  const error = levelError ?? rowsError;
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar ready={view} screen="list" badges={badges} end={<RowEnd list={list} decisions={state.decisions} total={kept?.length} />} />
      {error === undefined ? null : <p className="callout callout-bad">The list could not be read: {error}</p>}
      <Table list={list} decisions={state.decisions} lines={lines} columns={columns} context={context} selection={selection} selected={selected} everyLine={everyLine} lit={lit} />
      {kept !== undefined && level !== undefined && lines.length === 0 ? (
        <p className="muted row" data-testid="list-empty">
          Nothing matches at this detail, filter and search. <ResetView />
        </p>
      ) : null}
      {state.frontier ? <FrontierPanels view={view} decisions={state.decisions} drawn={drawn} /> : null}
    </section>
  );
}
