// C12: the decision view as data. The engine's projection is the level with only decisions
// shown, with each decision's answer in effect and the nodes that answer decides the relevance
// of; this turns it into the rows of what each answer affected, with titles, names, and each
// affected node's relevance now. Pure, so the unit tests check it without a page. (The graph with
// DECISIONS on draws the canvas itself, 5.1.)
//
// Cost: O(decisions + edges + affected nodes), with a map from key to node.
import type { Schema } from "@cairn/client";

import type { Relevance } from "../canvas/model.ts";
import { nodeOf, titleOf, type GraphNode, type NodeKind, type Ready, type State } from "../detail/model.ts";
import { answerText, entityName, roleTitle } from "../detail/sections.tsx";
import type { DisplayState } from "../status/words.ts";

export type DecisionView = Schema<"DecisionView">;
export type DecisionEntry = Schema<"DecisionEntry">;

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
