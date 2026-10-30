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

import type { Selection } from "../canvas/selection.ts";
import { decisionRows } from "../decisions/model.ts";
import { rankPlaces, type Ready } from "../detail/model.ts";
import { useMineOf } from "../journeys/mine.ts";
import { tableQueryOf, type LevelRequest } from "../journeys/requests.ts";
import type { ViewState } from "../journeys/view.ts";
import { containerKeys, planOrder } from "../plan/tree.ts";
import { Menu } from "../screens/Menu.tsx";
import { journeySubject, NothingMatches, ViewBar } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { DIFF_LABELS, type DiffMark } from "../proposals/model.ts";
import { useLayer, useReviewing, withRemovedRows } from "../proposals/layer.ts";
import { withListSettings, type ListSettings } from "./address.ts";
import { Checks } from "./Controls.tsx";
import type { NodeRow } from "./why.ts";
import { Cell, type CellContext } from "./ListCells.tsx";
import { Folds } from "./Folds.tsx";
import { NextForYou } from "./NextForYou.tsx";
import { StalledPanel } from "./Stalled.tsx";
import { addableColumns, columnsOf, columnWords, flatLines, givesWay, isDecisionColumn, sortOf, treeLines, type Column, type Line } from "./listModel.ts";
import { useRows } from "./useRows.ts";
import "./acting.css";
import { Failed } from "../ui/Failed.tsx";
import { PickBox } from "../ui/kit.tsx";
import { plural } from "../ui/words.ts";
import { useCaller } from "../data/react.ts";
import { useViewLevel } from "../canvas/viewLevel.ts";

/** The columns held at the left while the table scrolls sideways. */
function pinned(column: Column, columns: readonly Column[]): string | undefined {
  return column === "change" || column === "status" || column === "title" || (column === "rank" && columns.find((each) => each !== "change") === "rank") ? "list-pinned" : undefined;
}

/** A click on the row itself, not on a control inside it, selects it. */
function onRow(event: MouseEvent): boolean {
  return !(event.target instanceof Element && event.target.closest("a, button, select, input, textarea, label"));
}

function Head({ column, columns, list, onSort }: { column: Column; columns: readonly Column[]; list: ListSettings; onSort: (sort: ListSettings["sort"]) => void }) {
  const sort = sortOf(column);
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

/** C14: the change word of a marked row, and its open choice in accent ink. */
function Change({ mark }: { mark: DiffMark | undefined }) {
  if (mark === undefined) {
    return null;
  }
  return (
    <span className="layer-word" data-testid="row-diff" data-diff={mark.kind}>
      {DIFF_LABELS[mark.kind]}
    </span>
  );
}

function Row({ line, columns, context: shared, selection, current, faded, mark }: { line: Line; columns: Column[]; context: CellContext; selection: Selection; current: boolean; faded: boolean; mark: DiffMark | undefined }) {
  const { row } = line;
  // A removed row is read from the journey as it is: the one after has no such node.
  const context = line.from === undefined ? shared : { ...shared, view: line.from };
  const reviewing = useReviewing() !== undefined;
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
      data-diff={mark?.kind}
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
        {row === undefined || reviewing ? null : <PickBox className="list-checkbox" aria-label={`Select ${row.title}`} checked={selection.members.includes(line.key)} onChange={() => { selection.toggle(line.key); }} />}
      </td>
      {columns.map((column) => (
        <td key={column} data-column={column} data-gives-way={givesWay(column)} className={[column === "title" ? "list-title-cell" : "list-cell", pinned(column, columns)].filter(Boolean).join(" ")}>
          <div className="list-content">
            {column === "change" ? <Change mark={mark} /> : <Cell column={column} line={line} context={context} />}
          </div>
        </td>
      ))}
    </tr>
  );
}

/** The table: its header (the column labels) and its lines. */
function Table({ list, lines, columns, context, selection, selected, everyLine, outOfFocus, marks }: {
  marks: Record<string, DiffMark> | undefined;
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
    <table className="data list-table" data-selecting={selection.members.length > 1} data-ranked={columns.find((column) => column !== "change") === "rank"}
      data-layered={marks !== undefined} data-testid="list-table">
      <thead>
        <tr>
          <th scope="col" className="list-check list-pinned">
            {marks === undefined ? <PickBox className="list-checkbox" disabled={everyLine.length === 0} aria-label="Select every row shown" checked={all} onChange={() => { selection.set(all ? [] : everyLine); }} /> : null}
          </th>
          {columns.map((column) => (
            <Head key={column} column={column} columns={columns} list={list} onSort={(sort) => void navigate(`${pathname}${withListSettings(search, { ...list, sort })}`)} />
          ))}
        </tr>
      </thead>
      <tbody>
        {lines.map((line) => (
          <Row key={line.key} line={line} columns={columns} context={context} selection={selection} current={line.key === selected} faded={outOfFocus(line.key)} mark={marks?.[line.key]} />
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
        {total === undefined ? "Reading the list..." : plural(total, "item")}
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
  // The stalled panel above the rows already says what the journey waits on, so the line that says the same is left out.
  const stalled = view.derived.stalled != null;
  const entries = mine.status === "ready" ? mine.entries.filter((entry) => drawn?.has(entry.node) === true) : [];
  return (
    <>
      {!stalled ? <NextForYou view={view} entries={entries} /> : null}
      <Folds view={view} drawn={drawn} toOf={(key) => addresses.open(key)} />
    </>
  );
}

/**
 * C14: under the layer, the rows the proposal removes put back where they were, read from the
 * journey as it is: the level's entries and the rows with the removed ones added, their plan
 * order, and the keys that come from before. Without a layer the level's own, unchanged.
 */
function useRemovedRows({ view, layer, request, query, level, kept, order }: { view: Ready; layer: ReturnType<typeof useLayer>; request: LevelRequest | undefined; query: ReturnType<typeof tableQueryOf>; level: ReturnType<typeof useLevel>["level"]; kept: NodeRow[] | undefined; order: ReadonlyMap<string, number> }) {
  const was = layer?.mode === "reviewing" && layer.removed.size > 0 ? layer.before : undefined;
  const wasRows = useRows(was, query);
  const wasLevel = useLevel(was ?? view, (was ?? view).journey.graph.nodes ?? [], request);
  return useMemo(() => {
    if (was === undefined || kept === undefined || level === undefined || wasRows.rows === undefined || wasLevel.level === undefined || layer === undefined) {
      return { entries: level?.nodes, rows: kept, order, was: undefined, fromBefore: undefined };
    }
    const wasKept = wasRows.rows.filter((row) => wasLevel.drawn?.has(row.key) === true);
    const merged = withRemovedRows({ entries: level.nodes, rows: kept, was: { entries: wasLevel.level.nodes, rows: wasKept }, removed: layer.removed, order: planOrder(view), wasOrder: planOrder(was) });
    return { entries: merged.entries, rows: merged.rows, order: new Map(merged.order.map((key, at) => [key, at])), was, fromBefore: merged.fromBefore };
  }, [was, kept, level, wasRows.rows, wasLevel.level, wasLevel.drawn, layer, view, order]);
}

/**
 * PLAN, LIST: the rows the level draws, in the tree or sorted; the tree keeps the viewer's folds
 * across a filter and an emphasis; the rows checked are the journey's one selection.
 */
export function ListBody({ view, state, picks, list, selected, selection }: { view: Ready; state: ViewState; picks: number; list: ListSettings; selected?: string | undefined; selection: Selection }) {
  const nodes = view.journey.graph.nodes ?? [];
  const layer = useLayer();
  const { depth, step, current, request, level, drawn, error: levelError, base: frontier, dimmed, badges } = useViewLevel({ screen: "list", state, nodes, selected, ready: view, held: view });
  const query = useMemo(() => tableQueryOf(list.sort), [list.sort]);
  const { rows, error: rowsError } = useRows(view, query);
  const decisionColumns = depth === "decisions";
  const needsDecisions = decisionColumns || list.columns.some(isDecisionColumn);
  const projected = valueOf(useProjected(view, needsDecisions ? { projection: "decision_view" as const } : undefined));
  const decisions = useMemo(() => new Map((projected === undefined ? [] : decisionRows(view, projected)).map((decision) => [decision.key, decision])), [view, projected]);
  const ranks = rankPlaces(view);
  const order = useMemo(() => new Map(planOrder(view).map((key, at) => [key, at])), [view]);
  const containers = useMemo(() => containerKeys([...nodes, ...(layer?.before?.journey.graph.nodes ?? [])]), [nodes, layer]);
  const tree = list.sort === undefined;
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const { caller } = useCaller();
  const folds = useFolds(nodes, current, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const kept = useMemo(() => (rows === undefined || drawn === undefined ? undefined : rows.filter((row) => drawn.has(row.key))), [rows, drawn]);
  const { entries, rows: allRows, order: placed, was, fromBefore } = useRemovedRows({ view, layer, request, query, level, kept, order });
  const lines = useMemo(() => {
    if (allRows === undefined || entries === undefined) {
      return [];
    }
    const made = tree ? treeLines(entries, new Map(allRows.map((row) => [row.key, row])), folds.open, placed, containers) : flatLines(view, allRows, list.sort);
    return fromBefore === undefined ? made : made.map((line) => (fromBefore.has(line.key) ? { ...line, from: was } : line));
  }, [view, allRows, entries, tree, list.sort, folds.open, placed, containers, fromBefore, was]);
  const everyLine = lines.flatMap((line) => (line.row === undefined ? [] : [line.row.key]));
  const columns = columnsOf(list, decisionColumns, layer?.mode === "reviewing");
  const context: CellContext = { view, tree, decisions, ranks, frontier, sort: list.sort, viewer: caller?.identities ?? [], onFold: folds.fold };
  const error = levelError ?? rowsError;
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar subject={journeySubject(view)} screen="list" badges={badges} end={<RowEnd list={list} decisionColumns={decisionColumns} total={kept?.length} />} />
      {error === undefined ? null : <p className="callout callout-bad"><Failed what="The list" error={error} /></p>}
      {view.derived.stalled != null && layer === undefined ? <StalledPanel view={view} /> : null}
      <div className="node-panel list-panel" data-empty={lines.length === 0 ? true : undefined} data-testid="list-panel">
        <Table list={list} lines={lines} columns={columns} context={context} selection={selection} selected={selected} everyLine={everyLine} outOfFocus={dimmed} marks={layer?.mode === "reviewing" ? layer.model?.marks ?? {} : undefined} />
        {kept !== undefined && level !== undefined && lines.length === 0 ? (
          <div className="empty-row" data-testid="list-empty">
            <NothingMatches />
          </div>
        ) : null}
      </div>
      {state.frontier && layer === undefined ? <FrontierPanels view={view} drawn={drawn} /> : null}
    </section>
  );
}
