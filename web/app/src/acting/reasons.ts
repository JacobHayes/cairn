// C10: the one secondary fact a row or card says, in plain words: what the row needs
// ("Note required"), or the largest reason it ranks where it does ("2 days late", "Due in 3
// days", "Gates 6 nodes"). A conditional row says it may not apply. Nothing here is a code or a
// number without a word.
import { nodeOf, type Ready } from "../detail/model.ts";
import { dayOf } from "../timeline/model.ts";
import { missingEvidence } from "./acts.ts";
import type { SortBy } from "./address.ts";
import { rankParts, type NodeRow, type RankPart } from "./why.ts";

/** Within this share of the largest part, a date reason beats a bigger weight: it is the one to act on today. */
const CLOSE = 0.9;

const days = (count: number) => `${String(count)} ${count === 1 ? "day" : "days"}`;
const nodes = (count: number) => `${String(count)} ${count === 1 ? "node" : "nodes"}`;

/** How near the row's dates are, in words; none when it has no deadline. */
function dateReason(view: Ready, row: NodeRow): string | undefined {
  const today = dayOf(view.derived.today);
  if (row.overdue === true && row.due != null) {
    return `${days(today - dayOf(row.due))} late`;
  }
  const slack = row.slack_days;
  if (slack == null) {
    return undefined;
  }
  if (slack < 0) {
    return `Should have started ${days(-slack)} ago`;
  }
  if (slack === 0) {
    return "Start today";
  }
  return row.due == null ? `Start within ${days(slack)}` : `Due in ${days(Math.max(dayOf(row.due) - today, 0))}`;
}

/** What the row holds up: the nodes downstream of it, less itself; nothing when its gravity is only its own weight. */
function gravityReason(view: Ready, row: NodeRow): string | undefined {
  const from = view.derived.nodes[row.key]?.gravity_from;
  const downstream = from === undefined ? 0 : from.total - (from.entries.some((entry) => entry.node === row.key) ? 1 : 0);
  return downstream > 0 ? `Gates ${nodes(downstream)}` : undefined;
}

/** What finishing the row frees, naming others' work when some of it is. */
function leverageReason(view: Ready, row: NodeRow): string | undefined {
  const from = view.derived.nodes[row.key]?.leverage_from;
  const others = from?.entries.filter((entry) => entry.other_owner === true).length ?? 0;
  if (others > 0) {
    return `Unblocks ${String(others)} ${others === 1 ? "other's" : "others'"} work`;
  }
  return from === undefined || from.total === 0 ? undefined : `Unblocks ${nodes(from.total)}`;
}

/** The reason a rank part gives, in words. */
function partReason(view: Ready, row: NodeRow, part: RankPart): string | undefined {
  switch (part.signal) {
    case "urgency":
    case "late":
      return dateReason(view, row);
    case "gravity":
      return gravityReason(view, row);
    case "leverage":
      return leverageReason(view, row);
  }
}

/** The reason the largest rank part gives, a date reason when it is close to the largest; the next part's when a part has nothing to say. */
function rankReason(view: Ready, row: NodeRow): string | undefined {
  if (row.rank == null) {
    return dateReason(view, row);
  }
  const parts = rankParts(row.rank, view.inputs.rank);
  const top = parts[0];
  if (top === undefined) {
    return undefined;
  }
  const dated = parts.find((part) => (part.signal === "urgency" || part.signal === "late") && part.adds >= top.adds * CLOSE);
  const ordered = dated === undefined ? parts : [dated, ...parts.filter((part) => part !== dated)];
  return ordered.map((part) => partReason(view, row, part)).find((reason) => reason !== undefined);
}

/** What the row needs before it can be finished: a note or a link, or breaking down. */
function requirement(view: Ready, row: NodeRow): string | undefined {
  const node = nodeOf(view, row.key);
  if (node === undefined) {
    return undefined;
  }
  const needs = missingEvidence(view, node);
  if (needs.note) {
    return "Note required";
  }
  if (needs.artifact) {
    return "Link required";
  }
  return row.needs_breakdown === true ? "Needs breakdown" : undefined;
}

/**
 * The one fact under a row's title: that it may not apply (conditional), what it needs, or why
 * it ranks where it does, in that order of weight. When the list is sorted by one signal, that
 * signal's reason leads. None when the row has nothing worth saying.
 */
export function reasonOf(view: Ready, row: NodeRow, sort: SortBy = "rank"): string | undefined {
  if (row.display_state === "conditional") {
    return "May not apply";
  }
  const needed = requirement(view, row);
  if (needed !== undefined) {
    return needed;
  }
  switch (sort) {
    case "due":
    case "slack":
      return dateReason(view, row) ?? rankReason(view, row);
    case "gravity":
      return gravityReason(view, row) ?? rankReason(view, row);
    case "leverage":
      return leverageReason(view, row) ?? rankReason(view, row);
    case "rank":
    case "effort":
      return rankReason(view, row);
  }
}
