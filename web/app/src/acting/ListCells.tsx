// C9, C12: what each of the list's columns says about a row, in plain words: a status, a
// title with at most one thing under it, who owns it, when it is due. The columns Columns adds
// say a signal in words beside its number. A row with nothing to say in a column leaves it blank.
// With Frontier on, a row on the acting frontier says why it is there and carries its primary
// action under its title (C10, RowActions), at every width.
import { Link, useLocation } from "react-router";

import type { DecisionRow } from "../decisions/model.ts";
import { lateOf, type NodeKind, nodeOf, scheduledDate, type Ready, titleOf } from "../detail/model.ts";
import { screenSwitched, useNodeAddresses } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { NO_OWNER, ownerWords } from "../detail/sections.tsx";
import { snoozeWords } from "../status/snooze.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { dateInk, dateWords, daysWords, dueWords, inkClass } from "../status/when.ts";
import { Badge } from "../ui/kit.tsx";
import { TreeFold, TreeGuides } from "../ui/Tree.tsx";
import type { ListSettings } from "./address.ts";
import type { Column, Line } from "./listModel.ts";
import { firstParagraph } from "./listModel.ts";
import { factOf } from "./reasons.ts";
import { RowActions } from "./RowActions.tsx";
import { plural } from "../ui/words.ts";
import { NodeLink } from "../detail/parts.tsx";
import type { CSSProperties } from "react";
import { Tip } from "../ui/Tip.tsx";
import { Words } from "../ui/Words.tsx";

export interface CellContext {
  view: Ready;
  /** Whether the lines are the tree: its containers fold, and say how far along they are. */
  tree: boolean;
  /** Each decision's answer, rationale and what the answer affects (DECISIONS on). */
  decisions: Map<string, DecisionRow>;
  /** Each ranked node's place in the journey's rank order. */
  ranks: Map<string, number>;
  /** The acting frontier while Frontier is on: its rows say why they are there and carry their actions. */
  frontier: ReadonlySet<string> | undefined;
  sort: ListSettings["sort"];
  /** The viewer's identities, so an owner column says "You". */
  viewer: readonly string[];
  onFold: (container: string) => void;
}

/** C9: a row's container under its title, when the rows are not in the tree to show it. */
function Under({ view, line }: { view: Ready; line: Line }) {
  const ancestor = line.row?.ancestors?.at(-1);
  return ancestor === undefined ? null : (
    <span className="muted small" data-testid="breadcrumb">
      in <NodeLink view={view} node={ancestor} />
    </span>
  );
}

/** Where a snoozed row's snooze ends, in the card foot's words, as the row's one secondary fact (the Due column stays its due date). */
function Return({ view, line }: { view: Ready; line: Line }) {
  const derived = view.derived.nodes[line.key];
  const snoozed = derived === undefined ? undefined : snoozeWords(view, derived);
  return snoozed === undefined ? null : (
    <Tip tip={`${snoozed.words}${snoozed.tail ?? ""}`} kind="cut">
      <span className="muted small" data-testid="list-return">
        {snoozed.words}
        {snoozed.tail}
      </span>
    </Tip>
  );
}

/** D8: the date a scheduled row waits for, as its one secondary fact (the Due column stays its due date). */
function Scheduled({ view, line }: { view: Ready; line: Line }) {
  const date = scheduledDate(view, line.key);
  return date === undefined ? null : (
    <span className="muted small" data-testid="list-scheduled">
      {nodeOf(view, line.key)?.kind === "milestone" ? "On" : "Not before"} <Words text={dateWords(date, view.derived.today)} />
    </span>
  );
}

/** A node's due in words and ink: late as the engine counts it (a time that passed today is late by no days), else the distance to its date. */
function dueSaid(view: Ready, key: string, kind: NodeKind, fallback?: string): { words: string; ink: string | undefined } | undefined {
  const due = view.derived.nodes[key]?.dates.due?.date ?? fallback;
  if (due === undefined) {
    return undefined;
  }
  const late = lateOf(view, key);
  return { words: dueWords(due, view.derived.today, kind === "decision", late), ink: inkClass(dateInk(due, view.derived.today, late)) };
}

/** A phone drops the Due column, so the row says its due words under the title (a snoozed row says where the snooze ends instead, and a scheduled row the date it waits for: one secondary fact each). */
export function PhoneDue({ view, line, fact }: { view: Ready; line: Line; fact: string | undefined }) {
  const derived = view.derived.nodes[line.key];
  const node = nodeOf(view, line.key);
  const state = line.row?.display_state ?? derived?.display_state;
  const said = dueSaid(view, line.key, node?.kind ?? "action");
  const open = state !== undefined && state !== "done" && state !== "skipped" && state !== "not_relevant" && state !== "snoozed" && state !== "scheduled";
  if (!open || said === undefined || node === undefined) {
    return null;
  }
  // The fact under the title already says it when it is the due words ("14 days late").
  return said.words === fact ? null : <span className={`list-due-phone small ${said.ink ?? "muted"}`}><Words text={said.words} /></span>;
}

/** How far a container is, and the one thing beneath it that needs attention. */
function Progress({ view, line }: { view: Ready; line: Line }) {
  const roll = view.derived.nodes[line.key]?.progress;
  if (roll == null || roll.total === 0) {
    return null;
  }
  const attention = roll.overdue > 0 ? `${String(roll.overdue)} overdue` : roll.decisions_needed > 0 ? `${String(roll.decisions_needed)} to decide` : undefined;
  return (
    <span className="muted small" data-testid="progress">
      {roll.done} of {roll.total} done
      {attention === undefined ? null : <span className={roll.overdue > 0 ? "ink-bad" : ""}> · {attention}</span>}
    </span>
  );
}

/** C10: the one fact that says why a row is on the frontier, in words. When the fact is the row's due words, the Due column already says it beside the row, so only a phone (which has no such column, `PhoneDue`) shows it under the title. */
function Fact({ line, context }: { line: Line; context: CellContext }) {
  const { view } = context;
  const fact = line.row === undefined ? undefined : factOf(view, line.row, context.sort ?? "rank");
  const due = line.row?.due;
  const saidByColumn = fact !== undefined && due != null && fact.words === dueWords(due, view.derived.today, line.row?.kind === "decision", lateOf(view, line.key));
  return fact === undefined ? null : (
    <span className={`${fact.tone === undefined ? "muted small" : `small ink-${fact.tone}`}${saidByColumn ? " list-fact-due" : ""}`} data-testid="fact">
      <Words text={fact.words} />
    </span>
  );
}

/** C10: under a frontier row's title, why it is there and its primary action; the Owner column says who, so the action leaves the owner out unless there is none. */
function Acting({ line, context }: { line: Line; context: CellContext }) {
  const addresses = useNodeAddresses();
  return line.row === undefined ? null : (
    <span className="list-acting stack" data-testid="list-acting">
      <Fact line={line} context={context} />
      <RowActions view={context.view} row={line.row} to={addresses.open(line.key)} owner={false} column />
    </span>
  );
}

function Title({ line, context }: { line: Line; context: CellContext }) {
  const { view, tree } = context;
  const acting = context.frontier?.has(line.key) === true;
  return (
    <div className="list-title" style={{ "--depth": line.depth } as CSSProperties}>
      {tree ? (
        <>
          <TreeGuides depth={line.depth} guides={line.guides} open={line.open} />
          <TreeFold container={line.foldable} open={line.open} label={titleOf(view, line.key)} testId="list-fold" onFold={() => { context.onFold(line.key); }} />
        </>
      ) : null}
      <div className="list-node">
        <NodeLink view={view} node={line.key} />
        {acting ? <Acting line={line} context={context} /> : tree ? (line.container ? <Progress view={view} line={line} /> : null) : <Under view={view} line={line} />}
        <Return view={view} line={line} />
        <Scheduled view={view} line={line} />
        <PhoneDue view={view} line={line} fact={acting && line.row !== undefined ? factOf(view, line.row, context.sort ?? "rank")?.words : undefined} />
      </div>
    </div>
  );
}

/** What a node's affects say of its answer: what it brought in and ruled out, or what waits on it. */
function Affects({ context, node }: { context: CellContext; node: string }) {
  const { search } = useLocation();
  const target = useJourneyTarget();
  const decision = context.decisions.get(node);
  if (decision === undefined || decision.affects.length === 0) {
    return null;
  }
  const out = decision.affects.filter((each) => each.relevance === "not_relevant").length;
  const brought = decision.affects.length - out;
  const words =
    decision.answer === undefined
      ? `Decides ${plural(decision.affects.length, "item")}`
      : [brought > 0 ? `brings in ${String(brought)}` : "", out > 0 ? `drops ${String(out)}` : ""].filter(Boolean).join(", ");
  const kind = nodeOf(context.view, node);
  // The graph draws the node whatever the view hides (C7), so the link only has to name it.
  const to = kind === undefined ? undefined : screenSwitched(target, search, "graph", node);
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
  const open = derived?.open === true;
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
      return (row.owners ?? []).length > 0 ? ownerWords(view, row.owners, context.viewer) : row.unassigned === true ? <span className="list-unassigned">{NO_OWNER}</span> : null;
    case "due":
    {
      const said = row.due == null || !open ? undefined : dueSaid(view, line.key, node.kind, row.due);
      return said === undefined ? null : <span className={said.ink} data-testid="list-due"><Words text={said.words} /></span>;
    }
    case "rank": {
      const place = context.ranks.get(line.key);
      return place !== undefined ? `#${String(place)}` : open ? <Tip tip="Not on the frontier" kind="name" tap><span className="muted" role="img" aria-label="Not on the frontier">–</span></Tip> : null;
    }
    case "start_by": {
      const date = derived.dates.latest_start?.date;
      return date === undefined || !open ? null : <Words text={dateWords(date, today)} />;
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
      return rationale === undefined ? null : <Tip tip={firstParagraph(rationale)} kind="cut"><span className="list-why">{firstParagraph(rationale)}</span></Tip>;
    }
    case "affects":
      return <Affects context={context} node={line.key} />;
  }
}
