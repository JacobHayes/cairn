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
import { dependsWords } from "./words.ts";
import { NODE_GONE } from "../ui/gone.ts";

export type Level = Schema<"Level">;
export type LevelNode = Schema<"LevelNode">;
export type LevelEdge = Schema<"LevelEdge">;
export type Relevance = Schema<"Relevance">;
export type LevelDisplay = Schema<"LevelDisplay">;
export type Trace = Schema<"Trace">;
export type Stalled = Schema<"Stalled">;

/** C2: every kind, in the order the toggles list them; all shown by default. */
export const KINDS: NodeKind[] = ["group", "decision", "deliverable", "action", "milestone"];

/** C5: how many of the acting frontier's top-ranked items carry a rank tag on the canvas (the Signals lens shows the rest). */
export const RANK_TAG_COUNT = 3;

/** C6: a due date this many days away or fewer, and not yet late, is marked as coming up. */
export const DUE_SOON_DAYS = 7;

/** C2: Depth's steps, coarsest first, each named after the deepest kind it draws (5.1). */
export const STEPS = ["stages", "decisions", "deliverables", "actions"] as const;
export type Step = (typeof STEPS)[number];

/** C6: what the Signals lens puts on each card (8.2). */
export const LENSES = ["rank", "gravity", "unlocks", "slack"] as const;
export type Lens = (typeof LENSES)[number];

/**
 * What a graph shows (C2, C4): `step` is the depth (none is its default), `open` and `shut` the
 * containers the viewer expanded and collapsed since, and `lens` the Signals lens. The graph of
 * a journey or a route reads its step from the page's view state (journeys/view.ts) and the rest
 * from its own settings (settings.ts).
 */
export interface CanvasSettings {
  step: Step | undefined;
  open: string[];
  shut: string[];
  lens: Lens | undefined;
}

export type Tone = "plain" | "good" | "warn" | "bad";

/** A small badge on a card: a container's roll-up (C2). */
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

/** What a route's card says instead of a state (8.11): its date rule in words, and an advisory notice (A20). */
export interface RouteMarks {
  /** The one foot line: "Due 14 days before Decision meeting". */
  foot: string | undefined;
  /** No chain links the node to the final milestone, so neither priority nor dates reach it. */
  unanchored: boolean;
}

/** One card on the canvas. Journey fields are absent on a route's graph, which has no state. */
export interface Card {
  key: string;
  kind: NodeKind;
  title: string;
  /** The card it is drawn in; none at the top level. */
  parent: string | undefined;
  /** A top-level group: a stage. */
  stage: boolean;
  /** It has children (it can be expanded and collapsed, C4). */
  /** Its subtree is rolled up into it: the level collapsed it (5.3). */
  collapsed: boolean;
  /** The hidden work that rolled up into it, as a peek on hover (C4). */
  checklist: ChecklistItem[];
  /** Every hidden node that rolled up into it, whatever hid it (C2); the trace marks the card for each. */
  rolledUp: string[];
  /** C2: hidden, unsatisfied prerequisites no drawn edge stands for. */
  hiddenPrerequisites: string[];
  journey: CardState | undefined;
  /** A route draft's marks; none on a journey. */
  route?: RouteMarks | undefined;
}

/** The one secondary line a card may carry (5.5): a decided decision's answer, a container's progress, or what a conditional node depends on. */
export type CardBody =
  | { kind: "answer"; text: string; rationale: string | undefined }
  | { kind: "progress"; done: number; total: number; badge: CardBadge | undefined }
  | { kind: "depends"; text: string };

/** What the Signals lens can put on a card (8.2): each is a number, or none when the card has no value. */
export interface Signals {
  rank: number | undefined;
  gravity: number | undefined;
  /** How many nodes finishing it frees (the acting frontier only), and the weighted value the tint follows. */
  unlocks: { count: number; weighted: number } | undefined;
  slackDays: number | undefined;
}

/** A journey card's state and derived values (C1, C5, C6, C2's roll-ups). */
export interface CardState {
  /** The engine's display state (D8). */
  state: DisplayState;
  /** Done or skipped: nothing is left to do. */
  finished: boolean;
  relevance: Relevance;
  /** The foot's one date, in words, with the tone it reads in, a tail that is never cut when the words are (a node name takes the ellipsis, not what follows it), and the full sentence for its tooltip when the words are short for it. */
  foot: { words: string; tone: Tone; tail?: string; title?: string } | undefined;
  /** The foot's owner: only when it is the viewer or missing. */
  owner: { words: string; missing: boolean } | undefined;
  /** Everyone who owns it, for the hover. */
  owners: string;
  body: CardBody | undefined;
  here: Here;
  /** C5: its place in the acting frontier's rank order. */
  rank: number | undefined;
  signals: Signals;
  /** C5: a stage holding the acting frontier or active work, which the Stages step opens (5.1). */
  current: boolean;
}

/** How an edge is drawn (5.6), strongest first: a requirement, a condition gate, a stage opening, or dates alone. */
export type LineKind = "requires" | "condition" | "stage_opening" | "dates";

/** One line on the canvas: an edge between two cards (C1). */
export interface Line {
  id: string;
  from: string;
  to: string;
  /** Every edge it stands for is implicit: drawn dotted (C1). */
  implicit: boolean;
  /** Some edge it stands for blocks. */
  gates: boolean;
  /** The strongest style among the edges it stands for. */
  kind: LineKind;
  /** How many edges it stands for. */
  count: number;
  /** Nothing it stands for still waits: every requirement behind it is finished. Set where journey state is known. */
  satisfied?: boolean;
  /** What hovering it says, in words. */
  sentence: string;
}

/** What a journey's canvas reads besides its level: the rank order and the viewer's items. */
export interface JourneyExtras {
  /** C5: the acting frontier in rank order (the next list). */
  ranked: string[];
  /** C5: the viewer's own items: every node they hold a participation on. */
  mine: string[];
  /** The nodes they own, which a card says with "You". */
  owned: string[];
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

/** C1: what one edge says, in words: why the dependent waits on the requirement. */
function sentenceOf(edge: Schema<"UnderlyingEdge">, nodes: Nodes, graph: GraphNode[]): string {
  const title = (key: string) => nodes.get(key)?.title ?? NODE_GONE;
  if (!edge.gates) {
    return `Dated from ${title(edge.requirement)}; does not block`;
  }
  switch (edge.origin) {
    case "explicit":
      return `${title(edge.dependent)} needs ${title(edge.requirement)}`;
    case "condition": {
      const dependent = nodes.get(edge.dependent);
      const depends = dependent === undefined ? undefined : dependsWords(graph, dependent);
      return depends === undefined ? `Applies only depending on ${title(edge.requirement)}` : `Applies only if ${depends}`;
    }
    case "stage_opening":
      return `${title(edge.dependent)} waits for ${title(edge.requirement)}, where it opens`;
  }
}

/** Which line style wins where several edges stand in one line: solid over condition over stage opening over dates. */
export const STRENGTH: Record<LineKind, number> = { requires: 3, condition: 2, stage_opening: 1, dates: 0 };

function kindOf(edge: Schema<"UnderlyingEdge">): LineKind {
  if (!edge.gates) {
    return "dates";
  }
  return edge.origin === "explicit" ? "requires" : edge.origin;
}

/**
 * The card that stands for node `key` on a level: the card itself, the card it is rolled up into,
 * or else the nearest card holding it (a folded container, or a drawn ancestor); none when nothing
 * drawn stands for it.
 */
function standIn(level: Level, nodes: Nodes): (key: string) => string | undefined {
  const cards = new Map(level.nodes.flatMap((at) => [at.key, ...(at.rolled_up ?? [])].map((key) => [key, at.key] as const)));
  return (key) => {
    const seen = new Set<string>();
    for (let at: string | null | undefined = key; at != null && !seen.has(at); at = nodes.get(at)?.parent) {
      seen.add(at);
      const card = cards.get(at);
      if (card !== undefined) {
        return card;
      }
    }
    return undefined;
  };
}

/** Whether card `inner` sits somewhere inside card `outer`, given each card's parent. */
function inside(parents: Map<string, string | null | undefined>, inner: string, outer: string): boolean {
  for (let at = parents.get(inner); at != null; at = parents.get(at)) {
    if (at === outer) {
      return true;
    }
  }
  return false;
}

/**
 * C1: the level's edges as lines, each saying in words what it stands for. An edge whose end is
 * hidden is drawn to the card that stands for it (where it is rolled up, or the container folded
 * over it); one that then joins a card to itself or to a card around it (a node's requirement on
 * its own child) is not drawn, and edges that land between the same two cards are one line. A line
 * never ends where no card is.
 */
export function linesOf(level: Level, graph: GraphNode[], finished?: (key: string) => boolean): Line[] {
  const nodes = nodesOf(graph);
  const cardFor = standIn(level, nodes);
  const parents = new Map(level.nodes.map((at) => [at.key, at.parent]));
  const joined = new Map<string, { from: string; to: string; underlying: Schema<"UnderlyingEdge">[]; implicit: boolean }>();
  for (const edge of level.edges) {
    const [from, to] = [cardFor(edge.from), cardFor(edge.to)];
    if (from === undefined || to === undefined || from === to || inside(parents, from, to) || inside(parents, to, from)) {
      continue;
    }
    const id = `${from}->${to}`;
    const before = joined.get(id);
    joined.set(id, { from, to, underlying: [...(before?.underlying ?? []), ...edge.underlying], implicit: (before?.implicit ?? true) && edge.implicit === true });
  }
  return [...joined.entries()].map(([id, line]) => {
    const kind = line.underlying.map(kindOf).reduce<LineKind>((best, each) => (STRENGTH[each] > STRENGTH[best] ? each : best), "dates");
    const strongest = line.underlying.filter((each) => kindOf(each) === kind).map((each) => sentenceOf(each, nodes, graph));
    return {
      id,
      from: line.from,
      to: line.to,
      implicit: line.implicit,
      gates: line.underlying.some((each) => each.gates),
      kind,
      count: line.underlying.length,
      ...(finished === undefined ? {} : { satisfied: line.underlying.every((each) => !each.gates || finished(each.requirement)) }),
      sentence: `${strongest.slice(0, 2).join("; ")}${strongest.length > 2 ? `; and ${String(strongest.length - 2)} more` : ""}`,
    };
  });
}

/** What a journey adds to each card: its state, and whether a checklist item is finished. */
export interface Looks {
  card(node: GraphNode, at: LevelNode): CardState;
  finished(key: string): boolean;
}

/** C2: the level's nodes as cards; on a journey, `looks` adds each card's state. */
export function cardsOf(level: Level, graph: GraphNode[], looks?: Looks): Card[] {
  const nodes = nodesOf(graph);
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
        stage: node.kind === "group" && node.parent == null,
        collapsed: level.collapsed?.includes(node.key) === true,
        // Work hidden by its kind is the card's checklist; a node of a shown kind that rolled up
        // was hidden for its relevance class or a collapsed container, not work to tick off.
        checklist: (at.rolled_up ?? []).flatMap((key) => {
          const item = nodes.get(key);
          return item === undefined || level.shown.includes(item.kind) ? [] : [{ key, title: item.title, kind: item.kind, done: looks?.finished(key) ?? false }];
        }),
        rolledUp: at.rolled_up ?? [],
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
