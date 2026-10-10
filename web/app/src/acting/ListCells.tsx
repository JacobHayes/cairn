// C9, C12: what each of the Plan list's columns says about a row, in plain words: a status, a
// title with at most one thing under it, who owns it, when it is due. The columns Columns adds
// say a signal in words beside its number. A row with nothing to say in a column leaves it blank.
import { Link } from "react-router";

import { ancestorsOf } from "../canvas/ladder.ts";
import { DEFAULT_VIEW, canvasPath, revealing } from "../canvas/settings.ts";
import type { DecisionRow } from "../decisions/model.ts";
import { nodeOf, type Ready } from "../detail/model.ts";
import { entityName } from "../detail/sections.tsx";
import { rollups } from "../plan/tree.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { dateWords } from "../timeline/model.ts";
import { daysWords, dueWords } from "../status/when.ts";
import { Badge } from "../ui/kit.tsx";
import type { Column, Line } from "./listModel.ts";
import { firstParagraph } from "./listModel.ts";
import { DetailLink } from "./Parts.tsx";

export interface CellContext {
  view: Ready;
  /** Whether the lines are the tree: its containers fold, and say how far along they are. */
  tree: boolean;
  /** Each decision's answer, rationale and what the answer affects (DECISIONS on). */
  decisions: Map<string, DecisionRow>;
  /** Each ranked node's place in the journey's rank order. */
  ranks: Map<string, number>;
  onFold: (container: string) => void;
}

/** C9: a row's container under its title, when the rows are not in the tree to show it. */
function Under({ view, line }: { view: Ready; line: Line }) {
  const ancestor = line.row?.ancestors?.at(-1);
  return ancestor === undefined ? null : (
    <span className="muted small" data-testid="breadcrumb">
      in <DetailLink view={view} node={ancestor} />
    </span>
  );
}

/** How far a container is, and the one thing beneath it that needs attention. */
function Progress({ view, line }: { view: Ready; line: Line }) {
  const roll = rollups(view).get(line.key);
  if (roll === undefined || roll.total === 0) {
    return null;
  }
  const attention = roll.overdue > 0 ? `${String(roll.overdue)} overdue` : roll.toDecide > 0 ? `${String(roll.toDecide)} to decide` : undefined;
  return (
    <span className="muted small" data-testid="progress">
      {roll.done} of {roll.total} done
      {attention === undefined ? null : <span className={roll.overdue > 0 ? "list-late" : ""}> · {attention}</span>}
    </span>
  );
}

function Title({ line, context }: { line: Line; context: CellContext }) {
  const { view, tree } = context;
  return (
    <div className="list-title" style={{ paddingInlineStart: `calc(${String(line.depth)} * var(--space-5))` }}>
      {tree ? (
        line.container ? (
          <button type="button" className="list-fold" aria-expanded={line.open} aria-label={`${line.open ? "Fold" : "Open"} ${nodeOf(view, line.key)?.title ?? line.key}`} data-testid="list-fold" onClick={() => { context.onFold(line.key); }}>
            {line.open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="list-fold" aria-hidden="true" />
        )
      ) : null}
      <div className="list-node">
        <DetailLink view={view} node={line.key} />
        {tree ? (line.container ? <Progress view={view} line={line} /> : null) : <Under view={view} line={line} />}
      </div>
    </div>
  );
}

/** What a node's affects say of its answer: what it brought in and ruled out, or what waits on it. */
function Affects({ context, node }: { context: CellContext; node: string }) {
  const decision = context.decisions.get(node);
  if (decision === undefined || decision.affects.length === 0) {
    return null;
  }
  const out = decision.affects.filter((each) => each.relevance === "not_relevant").length;
  const brought = decision.affects.length - out;
  const words =
    decision.answer === undefined
      ? `Decides ${String(decision.affects.length)} ${decision.affects.length === 1 ? "item" : "items"}`
      : [brought > 0 ? `brings in ${String(brought)}` : "", out > 0 ? `drops ${String(out)}` : ""].filter(Boolean).join(", ");
  const kind = nodeOf(context.view, node);
  const to = kind === undefined ? undefined : canvasPath(context.view.journey.header.id, revealing(DEFAULT_VIEW, { kind: kind.kind, ancestors: ancestorsOf(context.view.journey.graph.nodes ?? [], node) }, context.view.derived.nodes[node]?.display_state), node);
  return to === undefined ? <>{words}</> : <Link to={to} data-testid="affects-link">{words}</Link>;
}

function Weight({ count, weighted }: { count: number; weighted: number }) {
  return count === 0 ? null : (
    <>
      {count} <span className="muted small">· weight {Number(weighted.toFixed(1))}</span>
    </>
  );
}

/** The cell of `column` for `line`. */
export function Cell({ column, line, context }: { column: Column; line: Line; context: CellContext }) {
  const { view } = context;
  const { row } = line;
  const derived = view.derived.nodes[line.key];
  const node = nodeOf(view, line.key);
  const state = row?.display_state ?? derived?.display_state;
  const today = view.derived.today;
  const open = state !== "done" && state !== "skipped" && state !== "not_relevant";
  if (column === "status") {
    return state === undefined || node === undefined ? null : <Badge tone={statusTone(state)} data-testid="list-state" data-status={state}>{statusWord(state, node.kind)}</Badge>;
  }
  if (column === "title") {
    return <Title line={line} context={context} />;
  }
  if (row === undefined || derived === undefined || node === undefined) {
    return null;
  }
  switch (column) {
    case "owner":
      return (row.owners ?? []).length > 0 ? (row.owners ?? []).map((key) => entityName(view, key)).join(", ") : row.unassigned === true ? <span className="list-unassigned">Unassigned</span> : null;
    case "due":
      return row.due == null || !open ? null : <span className={row.overdue === true ? "list-late" : ""} data-testid="list-due">{dueWords(row.due, today, node.kind === "decision")}</span>;
    case "rank": {
      const place = context.ranks.get(line.key);
      return place === undefined ? null : `#${String(place)}`;
    }
    case "start_by": {
      const date = derived.dates.latest_start?.date;
      return date === undefined || !open ? null : dateWords(date, today);
    }
    case "slack":
      return row.slack_days == null || !open ? null : row.slack_days >= 0 ? daysWords(row.slack_days) : `${daysWords(-row.slack_days)} behind`;
    case "gravity":
      return (derived.subtree_gravity ?? row.gravity).toFixed(1);
    case "unlocks":
      return <Weight count={derived.unlocks_from.total} weighted={row.unlocks} />;
    case "kind":
      return row.kind;
    case "effort":
      return node.estimate == null ? null : daysWords(node.estimate);
    case "answer": {
      const decision = context.decisions.get(line.key);
      return decision === undefined ? null : (decision.answer ?? <span className="muted">Not decided</span>);
    }
    case "why": {
      const rationale = context.decisions.get(line.key)?.rationale;
      return rationale === undefined ? null : <span className="list-why" title={firstParagraph(rationale)}>{firstParagraph(rationale)}</span>;
    }
    case "affects":
      return <Affects context={context} node={line.key} />;
  }
}
