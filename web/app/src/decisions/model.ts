// C12: the decision view as data. The engine's projection is the level with only decisions
// shown (C2's rules decide its gating edges and hidden-prerequisites markers), with each
// decision's answer in effect and the nodes that answer decides the relevance of; this turns
// it into the canvas's level and the rows of what each answer affected, with titles, names,
// and each affected node's relevance now. Pure, so the unit tests check it without a page.
//
// Cost: O(decisions + edges + affected nodes), with a map from key to node.
import type { Schema } from "@cairn/client";

import { LAYER_GAP_PX, NODE_GAP_PX, type Placement } from "../canvas/layout.ts";
import type { Level, Relevance } from "../canvas/model.ts";
import { nodeOf, titleOf, type GraphNode, type NodeKind, type Ready, type State } from "../detail/model.ts";
import { answerText, entityName, roleTitle } from "../detail/sections.tsx";
import type { DisplayState } from "../status/words.ts";

export type DecisionView = Schema<"DecisionView">;
export type DecisionEntry = Schema<"DecisionEntry">;

/** C12: the decision view as a canvas level: its decisions and the gating edges between them. */
export function decisionLevel(view: DecisionView): Level {
  return {
    shown: ["decision"],
    nodes: view.decisions.map((entry) => ({ key: entry.node, display_state: entry.display_state, hidden_prerequisites: entry.hidden_prerequisites ?? [] })),
    edges: view.edges,
  };
}

/** The most columns the decision canvas sets ungated decisions in. */
export const GRID_COLUMN_COUNT_MAX = 4;

/**
 * C12, C15: where the decision canvas draws its cards. ELK's layered layout stacks
 * unconnected cards in one column; when no decision gates another (the common case: decisions
 * wait on work, not on each other), the cards are set in tree order in a grid of up to
 * `GRID_COLUMN_COUNT_MAX` columns instead, as nearly square as the count allows. With any
 * gating edge, ELK's layout stands, so the edges keep their layered order. Deterministic: the
 * same cards always land in the same cells.
 */
export function decisionPlacement(keys: string[], placement: Placement, edgeCount: number): Placement {
  const placed = keys.flatMap((key) => {
    const at = placement[key];
    return at === undefined ? [] : [{ key, at }];
  });
  if (edgeCount > 0 || placed.length === 0) {
    return placement;
  }
  const columns = Math.min(GRID_COLUMN_COUNT_MAX, Math.ceil(Math.sqrt(placed.length)));
  const width = Math.max(...placed.map((each) => each.at.width)) + LAYER_GAP_PX;
  const gridded: Placement = {};
  let top = 0;
  for (let start = 0; start < placed.length; start += columns) {
    const row = placed.slice(start, start + columns);
    row.forEach((each, column) => {
      gridded[each.key] = { ...each.at, x: column * width, y: top };
    });
    top += Math.max(...row.map((each) => each.at.height)) + NODE_GAP_PX;
  }
  return gridded;
}

/** A node whose relevance an answer decides, and its relevance now. */
export interface Affected {
  key: string;
  title: string;
  kind: NodeKind | undefined;
  relevance: Relevance | undefined;
  /** D8: what it shows now: a node pending on another decision is conditional, not ruled out. */
  displayState: DisplayState | undefined;
}

/** C12: one decision, its answer in effect, and what that answer affected. */
export interface DecisionRow {
  key: string;
  title: string;
  prompt: string | undefined;
  /** Its stored state: what the answer was recorded in. */
  state: State;
  /** D8: the state it shows. */
  displayState: DisplayState;
  relevance: Relevance;
  /** The answer in effect (decided and in scope, E3), as people read it. */
  answer: string | undefined;
  /** B2: the markdown reason the answer in effect was given with. */
  rationale: string | undefined;
  /** Its owners by name; empty when unassigned. */
  owners: string[];
  /** The nodes whose relevance its answer decides, their descendants included. */
  affects: Affected[];
  /** The milestone its answer pins (E3), with that milestone's effective date. */
  pins: { key: string; title: string; date: string | undefined } | undefined;
  /** The role its answer fills (E3), by its title. */
  fills: string | undefined;
  /** Its unsatisfied prerequisites that no edge between decisions stands for. */
  waitsOn: { key: string; title: string }[];
}

function affectedOf(ready: Ready, key: string): Affected {
  return {
    key,
    title: titleOf(ready, key),
    kind: nodeOf(ready, key)?.kind,
    relevance: ready.derived.nodes[key]?.relevance.value,
    displayState: ready.derived.nodes[key]?.display_state,
  };
}

function rowOf(ready: Ready, entry: DecisionEntry, node: GraphNode | undefined): DecisionRow {
  const pinned = entry.pins ?? undefined;
  return {
    key: entry.node,
    title: titleOf(ready, entry.node),
    prompt: node?.prompt,
    state: entry.state,
    displayState: entry.display_state,
    relevance: entry.relevance,
    answer: entry.answer == null ? undefined : answerText(ready, entry.answer, node),
    rationale: entry.rationale ?? undefined,
    owners: (entry.owners ?? []).map((owner) => entityName(ready, owner)),
    affects: (entry.affects ?? []).map((key) => affectedOf(ready, key)),
    pins:
      pinned === undefined
        ? undefined
        : { key: pinned, title: titleOf(ready, pinned), date: ready.derived.nodes[pinned]?.dates.effective_date?.date },
    fills: entry.fills == null ? undefined : roleTitle(ready, entry.fills),
    waitsOn: (entry.hidden_prerequisites ?? []).map((key) => ({ key, title: titleOf(ready, key) })),
  };
}

/** C12: each decision of the view, in its order (tree order), with what its answer affected. */
export function decisionRows(ready: Ready, view: DecisionView): DecisionRow[] {
  return view.decisions.map((entry) => rowOf(ready, entry, nodeOf(ready, entry.node)));
}
