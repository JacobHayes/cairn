// C1 to C7: what the canvas draws, as data. One level of a graph (C2, the engine's `level`:
// which nodes are visible, under which container, what rolls up into each, which edges are
// drawn between which stand-ins, and the hidden-prerequisites markers) becomes cards and
// lines; on a journey each card also carries its state, owner, due date, decision answer,
// relevance look, priority weight, and "I am here" marks. The roll-up rules stay in the
// engine; this only reads them. Pure, so the unit tests check every rule without a canvas.
//
// Cost: O(nodes + edges) per level, with a map from key to node and to derived values.
import type { Schema } from "@cairn/client";

import type { GraphNode, NodeKind } from "../detail/model.ts";
import type { DisplayState } from "../status/words.ts";

export type Level = Schema<"Level">;
export type LevelNode = Schema<"LevelNode">;
export type LevelEdge = Schema<"LevelEdge">;
export type Relevance = Schema<"Relevance">;
export type Trace = Schema<"Trace">;
export type Stalled = Schema<"Stalled">;

/** C2: every kind, in the order the toggles list them; all shown by default. */
export const KINDS: NodeKind[] = ["group", "decision", "deliverable", "action", "milestone"];

/** C5: how many of the acting frontier's top-ranked items carry a numbered badge. */
export const RANK_BADGE_COUNT = 5;

/** C6: a due date this many days away or fewer, and not yet late, is marked as coming up. */
export const DUE_SOON_DAYS = 7;

/** C6: the border weights gravity maps onto, lightest first (px). */
export const BORDER_WEIGHTS_PX = [1, 2, 3, 4] as const;

/** What the canvas shows (C1, C2, C4, C6): the toggles and the container drilled into. */
export interface CanvasSettings {
  shown: NodeKind[];
  container: string | undefined;
  notRelevant: boolean;
  undecided: boolean;
  heat: boolean;
}

export const DEFAULT_SETTINGS: CanvasSettings = {
  shown: KINDS,
  container: undefined,
  notRelevant: true,
  undecided: true,
  heat: false,
};

export type Tone = "plain" | "good" | "warn" | "bad";

/** A small badge on a card: a derived flag or a container's roll-up (C2). */
export interface CardBadge {
  flag: string;
  tone: Tone;
}

/** A hidden node rolled up into a card, as a checklist item (C4). */
export interface ChecklistItem {
  key: string;
  title: string;
  kind: NodeKind;
  done: boolean;
}

/** C5: what puts a card on "I am here". */
export interface Here {
  frontier: boolean;
  active: boolean;
  /** `active` while still blocked (Gating: started early). */
  startedEarly: boolean;
  mine: boolean;
}

/** One card on the canvas. Journey fields are absent on a route's graph, which has no state. */
export interface Card {
  key: string;
  kind: NodeKind;
  title: string;
  /** The card it is drawn in; none at the top level. */
  parent: string | undefined;
  /** It has children with a stand-in (it may be drilled into, C4). */
  drillable: boolean;
  /** A decision's prompt (C1). */
  prompt: string | undefined;
  checklist: ChecklistItem[];
  /** C2: hidden, unsatisfied prerequisites no drawn edge stands for. */
  hiddenPrerequisites: string[];
  journey: CardState | undefined;
}

/** A journey card's state and derived values (C1, C5, C6, C2's roll-ups). */
export interface CardState {
  /** The engine's display state (D8). */
  state: DisplayState;
  /** Done or skipped: nothing is left to do. */
  finished: boolean;
  owner: string;
  relevance: Relevance;
  due: { date: string; tone: Tone } | undefined;
  latestStart: string | undefined;
  slackDays: number | undefined;
  answer: string | undefined;
  /** C6: the border weight gravity gives it (px). */
  borderPx: number;
  gravity: number;
  leverage: number;
  here: Here;
  /** C5: its place in the acting frontier's rank order, for the top few. */
  rank: number | undefined;
  badges: CardBadge[];
  /** C2: a container's children: the most gravity, the least slack, their owners. */
  children: { gravity: number | undefined; slackDays: number | undefined; owners: string[] } | undefined;
}

/** An implicit edge's source: a condition gate or a stage opening, in words. */
export interface LineSource {
  origin: "condition" | "stage_opening";
  words: string;
}

/** One line on the canvas: an edge between two cards (C1). */
export interface Line {
  id: string;
  from: string;
  to: string;
  /** Every edge it stands for is implicit: drawn dotted (C1). */
  implicit: boolean;
  /** Some edge it stands for blocks. */
  gates: boolean;
  /** Where each implicit edge it stands for comes from (Containment: named by its source). */
  sources: LineSource[];
}

/** What a journey's canvas reads besides its level: the rank order and the viewer's items. */
export interface JourneyExtras {
  /** C5: the acting frontier in rank order (the next list). */
  ranked: string[];
  /** C5: the viewer's own items. */
  mine: string[];
}

/** A canvas's cards and lines. */
export interface CanvasModel {
  cards: Card[];
  lines: Line[];
}

type Nodes = Map<string, GraphNode>;

function nodesOf(graph: GraphNode[]): Nodes {
  return new Map(graph.map((node) => [node.key, node]));
}

/** C1: where an implicit edge comes from, in words. */
function sourceOf(edge: Schema<"UnderlyingEdge">, nodes: Nodes): LineSource | undefined {
  const title = (key: string) => nodes.get(key)?.title ?? key;
  switch (edge.origin) {
    case "explicit":
      return undefined;
    case "condition":
      return { origin: "condition", words: `${title(edge.dependent)} is relevant by the answer to ${title(edge.requirement)}` };
    case "stage_opening":
      return { origin: "stage_opening", words: `${title(edge.dependent)} opens at ${title(edge.requirement)}` };
  }
}

/** C1: the level's edges as lines, implicit ones named by their source. */
export function linesOf(level: Level, graph: GraphNode[]): Line[] {
  const nodes = nodesOf(graph);
  return level.edges.map((edge) => ({
    id: `${edge.from}->${edge.to}`,
    from: edge.from,
    to: edge.to,
    implicit: edge.implicit === true,
    gates: edge.gates,
    sources: edge.underlying.flatMap((each) => sourceOf(each, nodes) ?? []),
  }));
}

/** What a journey adds to each card: its state, and whether a checklist item is finished. */
export interface Looks {
  card(node: GraphNode, at: LevelNode): CardState;
  finished(key: string): boolean;
}

/** C2: the level's nodes as cards; on a journey, `looks` adds each card's state. */
export function cardsOf(level: Level, graph: GraphNode[], looks?: Looks): Card[] {
  const nodes = nodesOf(graph);
  const parents = new Set(graph.flatMap((node) => (node.parent == null ? [] : [node.parent])));
  return level.nodes.flatMap((at) => {
    const node = nodes.get(at.key);
    if (node === undefined) {
      return [];
    }
    return [
      {
        key: node.key,
        kind: node.kind,
        title: node.title,
        parent: at.parent ?? undefined,
        drillable: parents.has(node.key),
        prompt: node.kind === "decision" ? node.prompt : undefined,
        checklist: (at.rolled_up ?? []).flatMap((key) => {
          const item = nodes.get(key);
          return item === undefined ? [] : [{ key, title: item.title, kind: item.kind, done: looks?.finished(key) ?? false }];
        }),
        hiddenPrerequisites: at.hidden_prerequisites ?? [],
        journey: looks?.card(node, at),
      },
    ];
  });
}

/** C6: the due badge's tone: late, coming up within `DUE_SOON_DAYS`, or plain. */
export function dueTone(due: string, today: string, overdue: boolean, finished: boolean): Tone {
  if (finished) {
    return "plain";
  }
  if (overdue) {
    return "bad";
  }
  const days = Math.round((Date.parse(due) - Date.parse(today)) / 86_400_000);
  return days <= DUE_SOON_DAYS ? "warn" : "plain";
}

/** C6: the border weight for `gravity` against the largest open gravity in the journey. */
export function borderFor(gravity: number, gravityMax: number, open: boolean): number {
  if (!open || gravityMax <= 0) {
    return BORDER_WEIGHTS_PX[0];
  }
  const step = Math.round((gravity / gravityMax) * (BORDER_WEIGHTS_PX.length - 1));
  return BORDER_WEIGHTS_PX[Math.min(Math.max(step, 0), BORDER_WEIGHTS_PX.length - 1)] ?? BORDER_WEIGHTS_PX[0];
}

