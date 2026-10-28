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

import { nodeTitle } from "../ui/gone.ts";
import { isStage } from "../detail/model.ts";
import { edgeWords } from "./words.ts";

export type Level = Schema<"Level">;
export type LevelNode = Schema<"LevelNode">;
export type LevelEdge = Schema<"LevelEdge">;
export type LevelDateLink = Schema<"LevelDateLink">;
export type DateLink = Schema<"DateLink">;
export type Relevance = Schema<"Relevance">;
export type LevelDisplay = Schema<"LevelDisplay">;
export type Trace = Schema<"Trace">;

/** C5: how many of the acting frontier's top-ranked items carry a rank tag on the canvas (the Signals lens shows the rest). */
export const RANK_TAG_COUNT = 3;

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

/** C5: what puts a card on "I am here". */
export interface Here {
  frontier: boolean;
  active: boolean;
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
  /** Every hidden node that rolled up into it, whatever hid it (C2); the trace marks the card for each. */
  rolledUp: string[];
  /** C2: hidden, unsatisfied prerequisites no drawn edge stands for. */
  hiddenPrerequisites: string[];
  /** C9: the search or filters match this node itself, not only something it holds. */
  matched: boolean;
  journey: CardState | undefined;
  /** A route draft's marks; none on a journey. */
  route?: RouteMarks | undefined;
}

/** The one secondary line a card may carry (5.5): a decided decision's answer, a container's progress, a childless piece of work's checklist count, or what a conditional node depends on. */
export type CardBody =
  | { kind: "answer"; text: string; rationale: string | undefined }
  | { kind: "progress"; done: number; total: number; badge: CardBadge | undefined }
  /** G5: work that holds no other nodes, how far its checklist is ticked. */
  | { kind: "checklist"; ticked: number; total: number }
  /** A group marked done: in place of its progress, how many items it skipped, or that kept work inside is still pending. */
  | { kind: "marked_done"; skipped: number; keptPending: boolean }
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

/** How an edge is drawn (5.6), strongest first: a requirement, a condition gate, or a stage opening. Every one holds its dependent back; date relationships are not edges (`DateLine`). */
export type LineKind = "requires" | "condition" | "stage_opening";

/** The kinds of line, strongest first. */
export const LINE_KINDS: readonly LineKind[] = ["requires", "condition", "stage_opening"];

/**
 * One edge of the graph a line stands for (C1): `dependent` waits on `requirement`. Where the
 * journey's state is known it also says whether the edge still holds its dependent back.
 */
export interface LineLink {
  requirement: string;
  dependent: string;
  kind: LineKind;
  /** The dependent holds nodes: a requirement on it is a gate on everything inside it. */
  holds: boolean;
  /** The edge still holds its dependent back, by the engine's own rule; none where there is no state (a route's). */
  waits?: boolean;
  /** What the edge says, in words with plain titles: the line's hover. */
  sentence: string;
}

/** One line on the canvas: the graph's edges between two cards (C1), kept so its card can list each. */
export interface Line {
  id: string;
  from: string;
  to: string;
  /** The edges it stands for, strongest kind first, then in the engine's order. */
  edges: LineLink[];
  /** The strongest style among the edges it stands for. */
  kind: LineKind;
  /** How many edges it stands for. */
  count: number;
  /** Nothing it stands for still waits. Set where journey state is known. */
  satisfied?: boolean;
  /** What hovering it says, in words. */
  sentence: string;
  /** Its card can be opened: the view's base level draws it (set where the model is composed). */
  selectable: boolean;
}

/**
 * C1: the date relationships between two cards (a due-by or not-before rule, a stage's close,
 * an opening that holds dates only, a date decision that sets a milestone), kept for the
 * selected node to draw. Not an edge: nothing waits on it.
 */
export interface DateLine {
  id: string;
  from: string;
  to: string;
  /** Some relationship it stands for fixes the order: `to` can never come before `from`. */
  orders: boolean;
  /** The relationships it stands for, each with its words. */
  entries: { link: DateLink; words: string }[];
  /** What hovering it says, in words. */
  sentence: string;
}

/** The cards a line joins, as an address names them (`<from>~<to>`); none unless it names exactly two. */
export function endsOf(edge: string): { from: string; to: string } | undefined {
  const [from, to, ...rest] = edge.split("~");
  return from === undefined || to === undefined || from === "" || to === "" || rest.length > 0 ? undefined : { from, to };
}

/** The id a line between cards `from` and `to` has in a model. */
export function lineIdOf(from: string, to: string): string {
  return `${from}->${to}`;
}

/** What a journey's canvas reads besides its level: the viewer's items. */
export interface JourneyExtras {
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

/** Which line style wins where several edges stand in one line: solid over condition over stage opening. */
export const STRENGTH: Record<LineKind, number> = { requires: 3, condition: 2, stage_opening: 1 };

function kindOf(edge: Schema<"UnderlyingEdge">): LineKind {
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
 * C1: a line from the edges it stands for: the kind is the strongest of theirs, the count theirs,
 * it is satisfied where every edge has a state and none waits, and its hover says the strongest
 * kind's edges. The edges are ordered strongest kind first, then as given (a stable sort).
 */
export function lineOf(id: string, from: string, to: string, links: readonly LineLink[]): Line {
  const edges = [...links].sort((one, other) => STRENGTH[other.kind] - STRENGTH[one.kind]);
  const kind = edges[0]?.kind ?? "requires";
  const strongest = edges.filter((each) => each.kind === kind).map((each) => each.sentence);
  return {
    id,
    from,
    to,
    edges,
    kind,
    count: edges.length,
    ...(edges.some((each) => each.waits === undefined) ? {} : { satisfied: edges.every((each) => each.waits !== true) }),
    sentence: `${strongest.slice(0, 2).join("; ")}${strongest.length > 2 ? `; and ${String(strongest.length - 2)} more` : ""}`,
    selectable: true,
  };
}

/** A line that stands for lines between other cards, and the lines it stands for. */
export interface JoinedLine {
  line: Line;
  members: Line[];
}

/**
 * Lines re-targeted to the cards that stand for their ends (`standIn`, none where nothing does):
 * a line that then joins a card to itself is not drawn, and lines that land between the same two
 * cards are one, which keeps every edge its members stand for. A line whose ends do not change
 * stays the line it was. Reveal and the zoomed-out tiles both draw lines this way (C2).
 */
export function joinLines(lines: readonly Line[], standIn: (key: string) => string | undefined): JoinedLine[] {
  const joined = new Map<string, { from: string; to: string; members: Line[] }>();
  for (const line of lines) {
    const [from, to] = [standIn(line.from), standIn(line.to)];
    if (from === undefined || to === undefined || from === to) {
      continue;
    }
    const id = lineIdOf(from, to);
    const kept = joined.get(id) ?? { from, to, members: [] };
    joined.set(id, kept);
    kept.members.push(line);
  }
  return [...joined.entries()].map(([id, one]) => {
    const [only] = one.members;
    const unchanged = one.members.length === 1 && only !== undefined && only.id === id;
    // A joined line opens its card where any line it stands for does: the view's own decision about which lines are selectable is carried, not made again.
    return { line: unchanged ? only : { ...lineOf(id, one.from, one.to, one.members.flatMap((member) => member.edges)), selectable: one.members.some((member) => member.selectable) }, members: one.members };
  });
}

/**
 * The line between two cards that stand for what is inside them: every line of the model that
 * runs from inside `from` (or from it) to inside `to` (or to it), as one, with each edge it
 * stands for. A line between two tiles is this. None where no line runs between the two.
 */
export function rolledLine(model: CanvasModel, ends: { from: string; to: string }): Line | undefined {
  const parents = new Map(model.cards.map((card) => [card.key, card.parent]));
  const within = (key: string, outer: string): boolean => {
    for (let at: string | undefined = key, seen = 0; at !== undefined && seen <= parents.size; at = parents.get(at), seen += 1) {
      if (at === outer) {
        return true;
      }
    }
    return false;
  };
  const standIn = (key: string) => (within(key, ends.from) ? ends.from : within(key, ends.to) ? ends.to : undefined);
  const wanted = lineIdOf(ends.from, ends.to);
  return joinLines(model.lines, standIn).find((each) => each.line.id === wanted)?.line;
}

/**
 * C1: the level's edges as lines, each keeping the graph's edges it stands for. An edge whose end
 * is hidden is drawn to the card that stands for it (where it is rolled up, or the container
 * folded over it); one that then joins a card to itself or to a card around it (a node's
 * requirement on its own child) is not drawn, and edges that land between the same two cards are
 * one line. A line never ends where no card is. `waiting` says whether an edge still holds its
 * dependent back; a route has no state, so passes none.
 */
export function linesOf(level: Level, graph: GraphNode[], waiting?: (edge: Schema<"UnderlyingEdge">) => boolean): Line[] {
  const nodes = nodesOf(graph);
  const ends = retargeted(level, graph);
  const holders = new Set(graph.flatMap((node) => (node.parent == null ? [] : [node.parent])));
  const joined = new Map<string, { from: string; to: string; links: LineLink[] }>();
  for (const edge of level.edges) {
    const found = ends(edge.from, edge.to);
    if (found === undefined) {
      continue;
    }
    const { from, to } = found;
    const id = lineIdOf(from, to);
    const line = joined.get(id) ?? { from, to, links: [] };
    joined.set(id, line);
    for (const each of edge.underlying) {
      line.links.push(linkOf(each, nodes, holders, waiting));
    }
  }
  return [...joined.entries()].map(([id, line]) => lineOf(id, line.from, line.to, line.links));
}

/**
 * Where a relationship between two nodes of the level is drawn: between the cards that stand for
 * them (the card a node is rolled up into, or the nearest card holding it). None when either has
 * no card, or both land on one card or on a card and one inside it (a node's relationship with its
 * own child): nothing is drawn between those.
 */
export function retargeted(level: Level, graph: GraphNode[]): (from: string, to: string) => { from: string; to: string } | undefined {
  const cardFor = standIn(level, nodesOf(graph));
  const parents = new Map(level.nodes.map((at) => [at.key, at.parent]));
  return (from, to) => {
    const [first, second] = [cardFor(from), cardFor(to)];
    return first === undefined || second === undefined || first === second || inside(parents, first, second) || inside(parents, second, first) ? undefined : { from: first, to: second };
  };
}

function linkOf(edge: Schema<"UnderlyingEdge">, nodes: Nodes, holders: ReadonlySet<string>, waiting: ((edge: Schema<"UnderlyingEdge">) => boolean) | undefined): LineLink {
  const link = { requirement: edge.requirement, dependent: edge.dependent, kind: kindOf(edge), holds: holders.has(edge.dependent), sentence: "" };
  return {
    ...link,
    sentence: edgeWords(link, nodeTitle(nodes.get(edge.dependent)?.title), nodeTitle(nodes.get(edge.requirement)?.title)),
    ...(waiting === undefined ? {} : { waits: waiting(edge) }),
  };
}

/** What a journey adds to each card: its state. */
export interface Looks {
  card(node: GraphNode, at: LevelNode): CardState;
  /** Whether an edge still holds its dependent back, by the engine's own rule (the dependent's unsatisfied gates), not by what its requirement's state looks like. */
  waiting(edge: Schema<"UnderlyingEdge">): boolean;
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
        stage: isStage(node),
        collapsed: level.collapsed?.includes(node.key) === true,
        rolledUp: at.rolled_up ?? [],
        hiddenPrerequisites: at.hidden_prerequisites ?? [],
        matched: at.matched ?? false,
        journey: looks?.card(node, at),
      },
    ];
  });
}
