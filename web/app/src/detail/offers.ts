// C11, B2, B5, B6, D4: what the inspector offers for a node by its kind and state (design 6.4,
// 6.5): one primary action, at most one visible secondary, and an overflow menu listing only what
// applies, grouped by how often it is wanted. Pure, so the unit tests check each kind and state
// without a page.
import { accepts, isTerminal, isWork, nodeOf, storedSnooze, titleOf, type NodeDetail, type NodeMove, type Ready } from "./model.ts";

/** The action beside a node, on any surface: what a person does to it in one move. */
export type Act = "decide" | "done" | "done-with-evidence" | "start" | "break-down" | "mark-atomic" | "unsnooze";

export interface Offered {
  /** The one primary, none when the node is finished, does not apply, or is a stage. */
  primary: Act | undefined;
  secondary: Act[];
}

/**
 * B6: the node whose snooze Unsnooze lifts for `key`: its own snooze, else the container's
 * holding it (a node cannot be unsnoozed through a snooze it does not have; a container with a
 * snooze of its own and another above it lifts its own first). None when nothing holds it.
 */
export function unsnoozeOf(view: Ready, key: string): string | undefined {
  return storedSnooze(view, key) ? key : (view.derived.nodes[key]?.snoozed_via ?? undefined);
}

/** The one place that decides which actions a node offers, for every surface that shows them: rows, the inspector, the graph's menu, the folds. A decision awaiting its answer offers Decide. What a node waits on never takes an action away: order is never a guard. */
export function offered(view: Ready, key: string): Offered {
  const node = nodeOf(view, key);
  const derived = view.derived.nodes[key];
  if (node === undefined || derived === undefined) {
    return { primary: undefined, secondary: [] };
  }
  const can = (move: NodeMove) => accepts(derived, move);
  const snooze: Act[] = unsnoozeOf(view, key) === undefined ? [] : ["unsnooze"];
  if (derived.open !== true) {
    return { primary: undefined, secondary: [] };
  }
  if (node.kind === "group") {
    return { primary: undefined, secondary: snooze };
  }
  if (derived.needs_breakdown === true) {
    return { primary: "break-down", secondary: ["mark-atomic", ...snooze] };
  }
  switch (node.kind) {
    case "milestone":
      return { primary: can("complete") ? "done" : undefined, secondary: snooze };
    case "decision":
      return { primary: can("answer") ? "decide" : undefined, secondary: snooze };
    default:
      // A completion the guards refuse still goes ahead with the evidence they lack.
      return { primary: can("complete") ? "done" : can("complete_anyway") ? "done-with-evidence" : undefined, secondary: [...(can("start") ? ["start" as const] : []), ...snooze] };
  }
}

/** One entry of the overflow menu. */
export type MenuId =
  | "snooze"
  | "skip"
  | "mark-done"
  | "stop"
  | "done-anyway"
  | "reopen"
  | "edit-date"
  | "keep"
  | "include"
  | "rename"
  | "assign"
  | "weight"
  | "pin"
  | "break-down"
  | "show-in-graph"
  | "insert-segment"
  | "show-insertion"
  | "edit-node"
  | "copy-link";

export interface MenuItem<Id extends string = MenuId> {
  id: Id;
  label: string;
}

const LABELS: Record<MenuId, string> = {
  snooze: "Snooze…",
  skip: "Skip…",
  "mark-done": "Mark done…",
  stop: "Stop",
  "done-anyway": "Done anyway…",
  reopen: "Reopen",
  "edit-date": "Edit actual date…",
  keep: "Keep…",
  include: "Include anyway…",
  rename: "Rename",
  assign: "Assign…",
  weight: "Change weight…",
  pin: "Pin a date…",
  "break-down": "Break down…",
  "show-in-graph": "Show in graph",
  "insert-segment": "Insert segment here…",
  "show-insertion": "Show insertion",
  "edit-node": "Edit node…",
  "copy-link": "Copy link",
};

function items(ids: (MenuId | false)[], labels: Partial<Record<MenuId, string>> = {}): MenuItem[] {
  return ids.flatMap((id) => (id === false ? [] : [{ id, label: labels[id] ?? LABELS[id] }]));
}

/**
 * The `⋯` menu's groups, most wanted first, and only what applies: a decided decision lists
 * Reopen, then the shape of the node, then where to find it; a node that does not apply lists
 * Include anyway and where to find it. Where the page offers no write (`writable` off,
 * screens/writable.ts) only where to find it.
 */
export function menuOf(view: Ready, detail: NodeDetail, writable: boolean): MenuItem[][] {
  const { node, derived, record } = detail;
  const state = record.state;
  const kind = node.kind;
  // A container holds other nodes; an empty group holds none yet, though a segment may go into it.
  const container = detail.children.length > 0;
  const can = (move: NodeMove) => accepts(derived, move);
  const keptUnder = derived.skipped_by != null && state !== "skipped";
  // A group with unfinished work inside can be marked done; one that reads done, skipped or not relevant has nothing to mark.
  const markable = can("mark_done") && derived.open === true;
  const member = (view.journey.graph.insertions ?? []).some((insertion) => node.key in insertion.nodes);
  const where = (...extra: (MenuId | false)[]): MenuItem[] => items(["show-in-graph", member && "show-insertion", ...extra, "copy-link"]);
  const groups = (...all: MenuItem[][]) => all.filter((group) => group.length > 0);
  if (!writable) {
    return groups(where());
  }
  if (derived.display_state === "not_relevant") {
    // Undoing a skip or a mark done never needs a force include.
    return groups(items([can("reopen") && "reopen", "include"]), where());
  }
  if (state === "skipped") {
    return groups(items(["reopen"]), items(["rename"]), where());
  }
  if (isTerminal(state)) {
    const labels = kind === "decision" ? { reopen: "Reopen (clears the answer and its reason)" } : kind === "milestone" ? { reopen: "Reopen (clears the actual date)" } : {};
    return groups(items(["reopen", kind === "milestone" && "edit-date"], labels), items(kind === "decision" ? ["rename", "assign", "weight"] : ["rename"]), where(kind === "decision" && "edit-node"));
  }
  const branch = container && kind !== "decision";
  return groups(
    items(
      [
        derived.snoozable === true && "snooze",
        markable && "mark-done",
        can("skip") && "skip",
        can("stop") && "stop",
        can("complete_anyway") && "done-anyway",
        keptUnder && "keep",
      ],
      branch ? { snooze: "Snooze branch…", skip: "Skip branch…" } : {},
    ),
    items(["rename", "assign", kind !== "milestone" && "weight", "pin", isWork(node) && !container && derived.needs_breakdown !== true && "break-down", can("insert_segment") && "insert-segment"], {
      pin: kind === "decision" ? "Pin decide-by…" : kind === "milestone" ? "Pin date…" : "Pin a date…",
    }),
    where("edit-node"),
  );
}

/** What a node's context menu on the graph can ask for: the row's actions, the `⋯` entries, and where else to see the node. */
export type ContextId = MenuId | Act | "show-in-list" | "toggle-selection";

/** What each action says, wherever it is shown. */
export const ACT_LABELS: Record<Act, string> = {
  decide: "Decide",
  done: "Mark done",
  "done-with-evidence": "Mark done…",
  start: "Start",
  "break-down": "Break down",
  "mark-atomic": "Mark atomic",
  unsnooze: "Unsnooze",
};

/** What `act` says for `key`: Unsnooze names the container when its snooze is the one holding the node. */
export function actLabel(view: Ready, key: string, act: Act): string {
  const holder = act === "unsnooze" ? unsnoozeOf(view, key) : undefined;
  return holder === undefined || holder === key ? ACT_LABELS[act] : `${ACT_LABELS[act]} ${titleOf(view, holder)}`;
}

/**
 * The context menu's groups: the row's actions (a decision awaiting its answer says Decide),
 * then the `⋯` menu's groups as they are, with where to find the node being the graph's
 * counterpart: the list. On the canvas, where cards are selected, the last group adds the node
 * to the selection or takes it out (`member`: it is in it), which is how touch selects several.
 * Where the page offers no write (`writable` off) it lists no action, only where to find the node.
 */
export function contextMenuOf(view: Ready, detail: NodeDetail, writable: boolean, member?: boolean): MenuItem<ContextId>[][] {
  const key = detail.node.key;
  const offers = offered(view, key);
  const acts: Act[] = !writable ? [] : [...(offers.primary === undefined ? [] : [offers.primary]), ...offers.secondary];
  const row: MenuItem<ContextId>[] = acts.map((id) => ({ id, label: actLabel(view, key, id) }));
  const rest = menuOf(view, detail, writable).map((group) => group.flatMap((item): MenuItem<ContextId>[] => (item.id === "show-in-graph" ? [{ id: "show-in-list", label: "Show in list" }] : [item])));
  const select: MenuItem<ContextId>[] = member === undefined ? [] : [{ id: "toggle-selection", label: member ? "Remove from selection" : "Add to selection" }];
  return [row, ...rest, select].filter((group) => group.length > 0);
}
