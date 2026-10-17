// C10, C11, D3: the one secondary fact a row says, in plain words and as data: that it may not
// apply, how late or how near its date is, what it needs ("Note required"), or the largest
// reason it ranks where it does ("Gates 6 nodes"). The fact is structured (`ActionFact`) so a
// caller can tone it, and `factWords` says it. Every surface that shows a row's action-priority
// fact takes it from `actionFact`, so a row reads the same wherever it is listed.
import { nodeOf, type Ready } from "../detail/model.ts";
import { daysUntil, daysWords, dueWords, isSoon } from "../status/when.ts";
import { dateWords } from "../timeline/model.ts";
import { missingEvidence } from "./acts.ts";
import type { SortBy } from "./address.ts";
import { rankParts, type NodeRow, type RankPart } from "./why.ts";

/** Within this share of the largest part, a date reason beats a bigger weight: it is the one to act on today. */
const CLOSE = 0.9;

const nodes = (count: number) => `${String(count)} ${count === 1 ? "node" : "nodes"}`;

/** The date reason of a row with no pressing date: when it is due or must start, in words; none when it has no deadline. */
function dateReason(view: Ready, row: NodeRow): string | undefined {
  const slack = row.slack_days;
  if (slack == null) {
    return undefined;
  }
  return row.due == null ? `Start within ${daysWords(slack)}` : dueWords(row.due, view.derived.today);
}

/** What the row holds up: the nodes downstream of it, less itself; nothing when its gravity is only its own weight. */
function gravityReason(view: Ready, row: NodeRow): string | undefined {
  const from = view.derived.nodes[row.key]?.gravity_from;
  const downstream = from === undefined ? 0 : from.total - (from.entries.some((entry) => entry.node === row.key) ? 1 : 0);
  return downstream > 0 ? `Gates ${nodes(downstream)}` : undefined;
}

/** What finishing the row frees, naming others' work when some of it is. */
function unlocksReason(view: Ready, row: NodeRow): string | undefined {
  const from = view.derived.nodes[row.key]?.unlocks_from;
  const others = from?.entries.filter((entry) => entry.other_owner === true).length ?? 0;
  if (others > 0) {
    return `Unlocks ${String(others)} ${others === 1 ? "other's" : "others'"} work`;
  }
  return from === undefined || from.total === 0 ? undefined : `Unlocks ${nodes(from.total)}`;
}

/** The reason a rank part gives, in words. */
function partReason(view: Ready, row: NodeRow, part: RankPart): string | undefined {
  switch (part.signal) {
    case "urgency":
    case "late":
      return dateReason(view, row);
    case "gravity":
      return gravityReason(view, row);
    case "unlocks":
      return unlocksReason(view, row);
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

/** The ink a fact is said in: warning for date pressure, error for lateness (DESIGN, Dates), always with its words. */
export type FactTone = "warn" | "bad";

/**
 * The one fact under an action row, as data. `late` is overdue work (`days` late; a decision names
 * the date it was to be decided by), `behind` work past the latest start its chain allows, `due` a
 * dated deadline (`date`; `decision` when it is decided by it rather than due on it) and `start`
 * that the slack is spent. `needs` is what the row requires before it can finish and `reason` the
 * largest reason it ranks where it does, both already in `words`.
 */
export type ActionFact =
  | { kind: "conditional"; tone: undefined }
  | { kind: "late"; tone: "bad"; days: number; decideBy: string | undefined }
  | { kind: "behind"; tone: "bad"; days: number }
  | { kind: "start"; tone: "warn" }
  | { kind: "due"; tone: FactTone | undefined; date: string; decision: boolean }
  | { kind: "needs" | "reason"; tone: undefined; words: string };

/** The fact in words. */
export function factWords(fact: ActionFact, today: string): string {
  switch (fact.kind) {
    case "conditional":
      return "May not apply";
    case "late": {
      const late = `${daysWords(fact.days)} late`;
      return fact.decideBy === undefined ? late : `Decide by ${dateWords(fact.decideBy, today)}, ${late}`;
    }
    case "behind":
      return `Should have started ${daysWords(fact.days)} ago`;
    case "start":
      return "Start today";
    case "due":
      return dueWords(fact.date, today, fact.decision);
    case "needs":
    case "reason":
      return fact.words;
  }
}

/** The sorts whose own reason leads the fact, so a date that is merely near does not displace it. */
const SIGNAL_SORTS: ReadonlySet<SortBy> = new Set<SortBy>(["gravity", "unlocks"]);

/**
 * C10, C11, D3: the one fact under a row's title. In order of weight: that the row may not apply;
 * that it is late, or past the latest start its chain allows (whatever the list is sorted by);
 * that its slack is spent or its date is within the soon window (unless the list is sorted by
 * gravity or unlocks, which lead with their own reason); what the row needs; a decision's
 * date; then the reason the sort asks for, else why it ranks where it does. None when the row has
 * nothing worth saying.
 */
export function actionFact(view: Ready, row: NodeRow, sort: SortBy = "rank"): ActionFact | undefined {
  if (row.display_state === "conditional") {
    return { kind: "conditional", tone: undefined };
  }
  const today = view.derived.today;
  const decision = row.kind === "decision";
  if (row.overdue === true && row.due != null) {
    return { kind: "late", tone: "bad", days: -daysUntil(row.due, today), decideBy: decision ? row.due : undefined };
  }
  const slack = row.slack_days;
  if (slack != null && slack < 0) {
    return { kind: "behind", tone: "bad", days: -slack };
  }
  const led = SIGNAL_SORTS.has(sort);
  if (!led) {
    if (slack === 0) {
      return { kind: "start", tone: "warn" };
    }
    if (row.due != null && isSoon(row.due, today)) {
      return { kind: "due", tone: "warn", date: row.due, decision };
    }
  }
  const needed = requirement(view, row);
  if (needed !== undefined) {
    return { kind: "needs", tone: undefined, words: needed };
  }
  if (decision && row.due != null && !led) {
    return { kind: "due", tone: undefined, date: row.due, decision };
  }
  const words = reasonWords(view, row, sort);
  return words === undefined ? undefined : { kind: "reason", tone: undefined, words };
}

/** The reason the sort asks for, else why the row ranks where it does. */
function reasonWords(view: Ready, row: NodeRow, sort: SortBy): string | undefined {
  switch (sort) {
    case "due":
    case "slack":
      return dateReason(view, row) ?? rankReason(view, row);
    case "gravity":
      return gravityReason(view, row) ?? rankReason(view, row);
    case "unlocks":
      return unlocksReason(view, row) ?? rankReason(view, row);
    case "rank":
    case "effort":
      return rankReason(view, row);
  }
}

/** The fact and its tone, in words, for a row: what a list draws under a title. */
export function factOf(view: Ready, row: NodeRow, sort: SortBy = "rank"): { words: string; tone: FactTone | undefined } | undefined {
  const fact = actionFact(view, row, sort);
  return fact === undefined ? undefined : { words: factWords(fact, view.derived.today), tone: fact.tone };
}
