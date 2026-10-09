// C11, B2, B5, B6, D4: what the inspector offers for a node by its kind and state (design 6.4,
// 6.5): one primary action, at most one visible secondary, and an overflow menu listing only what
// applies, grouped by how often it is wanted. Pure, so the unit tests check each kind and state
// without a page.
import { missingEvidence } from "../acting/acts.ts";
import { breakable } from "../proposals/model.ts";
import { isBlocked, isTerminal, movesFrom, type NodeDetail, type Ready } from "./model.ts";

/** The action in the row beside the form: what a person does to the node in one move. */
export type Act = "done" | "done-with-evidence" | "start" | "reach" | "reach-now" | "break-down" | "mark-atomic" | "unsnooze";

export interface Offered {
  /** The one primary, none when the node is finished, does not apply, or is a stage. */
  primary: Act | undefined;
  secondary: Act[];
  /** The primary would be refused: something it waits on is not done (the `⋯` menu answers anyway). */
  waiting: boolean;
}

/** The primary and secondary actions of a node that is not a decision awaiting its answer. */
export function offered(view: Ready, detail: NodeDetail): Offered {
  const { node, derived, record } = detail;
  const shown = derived.display_state;
  const stored = view.journey.graph.state?.snoozes?.[node.key] !== undefined;
  const snooze: Act[] = stored ? ["unsnooze"] : [];
  if (shown === "done" || shown === "skipped" || shown === "not_relevant" || isTerminal(record.state)) {
    return { primary: undefined, secondary: [], waiting: false };
  }
  if (node.kind === "group") {
    return { primary: undefined, secondary: snooze, waiting: false };
  }
  const moves = movesFrom(node.kind, record.state);
  const waiting = isBlocked(derived, view);
  if (derived.needs_breakdown === true) {
    return { primary: "break-down", secondary: ["mark-atomic", ...snooze], waiting: false };
  }
  switch (node.kind) {
    case "milestone":
      return { primary: shown === "scheduled" ? "reach-now" : "reach", secondary: snooze, waiting };
    case "decision":
      return { primary: undefined, secondary: snooze, waiting };
    default: {
      if (!moves.includes("complete")) {
        return { primary: undefined, secondary: snooze, waiting: false };
      }
      const needs = missingEvidence(view, node);
      return {
        primary: needs.artifact || needs.note ? "done-with-evidence" : "done",
        secondary: [...(moves.includes("start") ? (["start"] as const) : []), ...snooze],
        waiting,
      };
    }
  }
}

/** One entry of the overflow menu. */
export type MenuId =
  | "snooze"
  | "skip"
  | "stop"
  | "done-anyway"
  | "reach-anyway"
  | "answer-anyway"
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
  | "edit-node"
  | "copy-link";

export interface MenuItem {
  id: MenuId;
  label: string;
}

const LABELS: Record<MenuId, string> = {
  snooze: "Snooze…",
  skip: "Skip…",
  stop: "Stop",
  "done-anyway": "Done anyway…",
  "reach-anyway": "Reach anyway…",
  "answer-anyway": "Answer anyway…",
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
  "edit-node": "Edit node…",
  "copy-link": "Copy link",
};

function items(ids: (MenuId | false)[], labels: Partial<Record<MenuId, string>> = {}): MenuItem[] {
  return ids.flatMap((id) => (id === false ? [] : [{ id, label: labels[id] ?? LABELS[id] }]));
}

/**
 * The `⋯` menu's groups, most wanted first, and only what applies: a decided decision lists
 * Reopen, then the shape of the node, then where to find it; a node that does not apply lists
 * Include anyway and where to find it.
 */
export function menuOf(view: Ready, detail: NodeDetail): MenuItem[][] {
  const { node, derived, record } = detail;
  const state = record.state;
  const kind = node.kind;
  const container = detail.children.length > 0 || kind === "group";
  const blocked = isBlocked(derived, view);
  const needs = missingEvidence(view, node);
  const guardFails = blocked || needs.artifact || needs.note || derived.needs_breakdown === true;
  const keptUnder = derived.effectively_skipped === true && state !== "skipped";
  const where = (...extra: (MenuId | false)[]): MenuItem[] => items(["show-in-graph", ...extra, "copy-link"]);
  const groups = (...all: MenuItem[][]) => all.filter((group) => group.length > 0);
  if (derived.display_state === "not_relevant") {
    return groups(items(["include"]), where());
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
        "snooze",
        "skip",
        state === "active" && "stop",
        kind === "decision" && blocked && "answer-anyway",
        kind === "milestone" && blocked && "reach-anyway",
        (kind === "deliverable" || kind === "action") && guardFails && "done-anyway",
        keptUnder && "keep",
      ],
      branch ? { snooze: "Snooze branch…", skip: "Skip branch…" } : {},
    ),
    items(["rename", "assign", kind !== "milestone" && "weight", "pin", breakable(node) && !container && derived.needs_breakdown !== true && "break-down"], {
      pin: kind === "decision" ? "Pin decide-by…" : kind === "milestone" ? "Pin date…" : "Pin a date…",
    }),
    where("edit-node"),
  );
}
